import SwiftUI
import UIKit

@main
class AppDelegate: UIResponder, UIApplicationDelegate {
  let webServer = GCDWebServer()
  var window: UIWindow?

  func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication
      .LaunchOptionsKey: Any]?
  ) -> Bool {
    createWindow()
    startWebServer()
    return true
  }
  
  func createWindow() {
    let contentView = ContentView()
    let window = UIWindow(frame: UIScreen.main.bounds)
    window.rootViewController = UIHostingController(rootView: contentView)
    self.window = window
    window.makeKeyAndVisible()
  }

  func startWebServer() {
    webServer.addDefaultHandler(
      forMethod: "GET",
      request: GCDWebServerRequest.self,
      processBlock: handleWebRequest)

    let options: [String: Any] = [
      "AutomaticallySuspendInBackground": false,
      "BindToLocalhost": false,
      "BonjourName": "Fitsme Server",
      "ConnectedStateCoalescingInterval": 2.0,
      "Port": 8080,
    ]

    try! webServer.start(options: options)
  }

  func handleWebRequest(request: GCDWebServerRequest)
    -> GCDWebServerResponse?
  {
    let resp = GCDWebServerStreamedResponse.init(
      contentType: "text/plain",
      asyncStreamBlock: { block in
        block("this is a block ".data(using: .utf8), nil)
      })
    return resp
  }
}
