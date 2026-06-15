import Cocoa
import UserNotifications

/// Handles macOS notifications for JustDrop file transfers.
///
/// Provides accept/reject prompts for incoming transfers,
/// progress updates, and completion alerts showing the saved path.
class TransferNotificationManager: NSObject, UNUserNotificationCenterDelegate {

    static let shared = TransferNotificationManager()

    private override init() {
        super.init()
    }

    /// Request notification permissions and set up categories.
    func setup() {
        let center = UNUserNotificationCenter.current()
        center.delegate = self

        // Define the accept/reject actions
        let acceptAction = UNNotificationAction(
            identifier: "ACCEPT_TRANSFER",
            title: "Accept",
            options: [.foreground]
        )
        let rejectAction = UNNotificationAction(
            identifier: "REJECT_TRANSFER",
            title: "Reject",
            options: [.destructive]
        )
        let transferCategory = UNNotificationCategory(
            identifier: "INCOMING_TRANSFER",
            actions: [acceptAction, rejectAction],
            intentIdentifiers: [],
            options: [.customDismissAction]
        )
        center.setNotificationCategories([transferCategory])

        center.requestAuthorization(options: [.alert, .sound, .badge]) { granted, error in
            if let error = error {
                NSLog("JustDrop: Notification permission error: \(error)")
            }
            NSLog("JustDrop: Notification permission granted: \(granted)")
        }
    }

    /// Show an incoming transfer request popup with Accept / Reject.
    func showIncomingRequest(
        transferId: String,
        senderName: String,
        fileCount: Int,
        totalSize: String
    ) {
        let content = UNMutableNotificationContent()
        content.title = "Incoming Transfer"

        if fileCount == 1 {
            content.body = "\(senderName) wants to send you a file (\(totalSize))"
        } else {
            content.body = "\(senderName) wants to send you \(fileCount) files (\(totalSize))"
        }

        content.sound = .default
        content.categoryIdentifier = "INCOMING_TRANSFER"
        content.userInfo = ["transfer_id": transferId]

        let request = UNNotificationRequest(
            identifier: "transfer-request-\(transferId)",
            content: content,
            trigger: nil
        )

        UNUserNotificationCenter.current().add(request)
    }

    /// Show transfer progress notification.
    func showProgress(
        transferId: String,
        fileName: String,
        percent: Int,
        speed: String
    ) {
        let content = UNMutableNotificationContent()
        content.title = "Receiving: \(fileName)"
        content.body = "\(percent)% • \(speed)"
        content.sound = nil

        let request = UNNotificationRequest(
            identifier: "transfer-progress-\(transferId)",
            content: content,
            trigger: nil
        )

        UNUserNotificationCenter.current().add(request)
    }

    /// Show transfer complete notification with saved file path.
    func showComplete(
        transferId: String,
        fileName: String,
        savedPath: String
    ) {
        // Remove progress notification
        UNUserNotificationCenter.current().removeDeliveredNotifications(
            withIdentifiers: ["transfer-progress-\(transferId)"]
        )

        let content = UNMutableNotificationContent()
        content.title = "Transfer Complete"
        content.body = "\(fileName) saved to ~/JustDrop"
        content.sound = .default

        let request = UNNotificationRequest(
            identifier: "transfer-complete-\(transferId)",
            content: content,
            trigger: nil
        )

        UNUserNotificationCenter.current().add(request)
    }

    /// Show transfer failed notification.
    func showFailed(transferId: String, reason: String) {
        let content = UNMutableNotificationContent()
        content.title = "Transfer Failed"
        content.body = reason
        content.sound = .default

        let request = UNNotificationRequest(
            identifier: "transfer-failed-\(transferId)",
            content: content,
            trigger: nil
        )

        UNUserNotificationCenter.current().add(request)
    }

    // MARK: - UNUserNotificationCenterDelegate

    /// Handle notification actions (accept/reject).
    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        let userInfo = response.notification.request.content.userInfo
        guard let transferId = userInfo["transfer_id"] as? String else {
            completionHandler()
            return
        }

        switch response.actionIdentifier {
        case "ACCEPT_TRANSFER":
            NSLog("JustDrop: User accepted transfer \(transferId)")
            // Forward to Rust engine
        case "REJECT_TRANSFER":
            NSLog("JustDrop: User rejected transfer \(transferId)")
            // Forward to Rust engine
        default:
            break
        }

        completionHandler()
    }

    /// Show notifications while the app is in the foreground.
    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        completionHandler([.banner, .sound])
    }
}
