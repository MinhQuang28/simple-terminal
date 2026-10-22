//! Posts notifications through the UserNotifications framework, the same one the permission
//! check reads. (The older NSUserNotificationCenter no longer delivers on current macOS.)

use std::cell::RefCell;

use block2::{DynBlock, RcBlock};
use objc2::rc::Retained;
use objc2::runtime::{Bool, NSObjectProtocol, ProtocolObject};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_foundation::{NSError, NSObject, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotification, UNNotificationPresentationOptions,
    UNNotificationRequest, UNNotificationSound, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};

use crate::notification_permission::NotificationPermission;

define_class!(
    // SAFETY: NSObject has no subclassing requirements and Delegate doesn't implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "RepodeckNotificationDelegate"]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    // Without a delegate macOS shows nothing while the app is frontmost, which is where the
    // settings page's test button and agents in background tabs post from.
    unsafe impl UNUserNotificationCenterDelegate for Delegate {
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            completion: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            let options = UNNotificationPresentationOptions::Banner
                | UNNotificationPresentationOptions::List
                | UNNotificationPresentationOptions::Sound;
            completion.call((options,));
        }
    }
);

thread_local! {
    static DELEGATE: RefCell<Option<Retained<Delegate>>> = const { RefCell::new(None) };
}

/// True when the framework can be used: inside the app bundle, from the main thread.
pub fn available() -> bool {
    NotificationPermission::available() && MainThreadMarker::new().is_some()
}

/// Posts a notification, asking macOS for permission first if it never has.
pub fn post(title: &str, body: &str, sound: bool) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let center = UNUserNotificationCenter::currentNotificationCenter();
    DELEGATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let delegate = slot.get_or_insert_with(|| {
            let this = Delegate::alloc(mtm).set_ivars(());
            // SAFETY: plain NSObject init on a class with no required initializer.
            unsafe { msg_send![super(this), init] }
        });
        center.setDelegate(Some(ProtocolObject::from_ref(&**delegate)));
    });

    let (title, body) = (title.to_string(), body.to_string());
    // Granting (or an earlier answer) comes back on a background queue; the request itself may
    // be added from there.
    let then_post = RcBlock::new(move |granted: Bool, _error: *mut NSError| {
        if !granted.as_bool() {
            log::info!("Notifications are not allowed by macOS");
            return;
        }
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(&title));
        content.setBody(&NSString::from_str(&body));
        if sound {
            content.setSound(Some(&UNNotificationSound::defaultSound()));
        }
        let id = NSString::from_str(&uuid::Uuid::new_v4().to_string());
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(&id, &content, None);
        let done = RcBlock::new(|error: *mut NSError| {
            // SAFETY: the framework passes either null or a live error for the handler's duration.
            if let Some(error) = unsafe { error.as_ref() } {
                log::warn!("Notification failed: {}", error.localizedDescription());
            }
        });
        UNUserNotificationCenter::currentNotificationCenter()
            .addNotificationRequest_withCompletionHandler(&request, Some(&done));
    });
    let options =
        UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound | UNAuthorizationOptions::Badge;
    center.requestAuthorizationWithOptions_completionHandler(options, &then_post);
}
