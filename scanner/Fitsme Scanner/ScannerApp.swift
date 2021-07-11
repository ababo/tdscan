import SwiftUI

@main
struct ScannerApp: App {
  let webServer = GCDWebServer()

  init() {
    webServer.addDefaultHandler(
      forMethod: "GET",
      request: GCDWebServerRequest.self,
      processBlock: handleRequest)
    startServer()
  }

  var body: some Scene {
    return WindowGroup {
      ContentView()
    }
  }

  func handleRequest(request: GCDWebServerRequest)
    -> GCDWebServerResponse?
  {
    let resp = GCDWebServerStreamedResponse.init(
      contentType: "text/plain",
      asyncStreamBlock: { block in
        block("this is a block ".data(using: .utf8), nil)
      })
    return resp
  }

  func startServer() {
    let options: [String: Any] = [
      "AutomaticallySuspendInBackground": false,
      "BindToLocalhost": false,
      "BonjourName": "Fitsme Server",
      "ConnectedStateCoalescingInterval": 2.0,
      "Port": 8080,
    ]
    try! webServer.start(options: options)
  }
}
