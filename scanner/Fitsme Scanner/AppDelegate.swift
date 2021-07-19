import ARKit
import SwiftUI
import UIKit

@main
class AppDelegate: UIResponder, UIApplicationDelegate {
  struct ScanState {
    let fps: Double
    var frameIndex: Int
    let numFrames: Int
    let start: TimeInterval
  }

  let session = ScanSession()
  let webServer = GCDWebServer()

  var scanLock = NSLock()
  var scanState: ScanState?
  var window: UIWindow?

  override init() {
    super.init()
    session.onFrame = onFrame
  }

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
    let contentView = ContentView(session: session)
    let window = UIWindow(frame: UIScreen.main.bounds)
    window.rootViewController = UIHostingController(rootView: contentView)
    self.window = window
    window.makeKeyAndVisible()
  }

  func startWebServer() {
    webServer.addHandler(
      forMethod: "GET", path: "/formats", request: GCDWebServerRequest.self,
      processBlock: handleFormatsRequest)

    webServer.addHandler(
      forMethod: "GET", path: "/scan",
      request: GCDWebServerRequest.self,
      processBlock: handleScanRequest)

    let options: [String: Any] = [
      "AutomaticallySuspendInBackground": false,
      "BindToLocalhost": false,
      "BonjourName": "Fitsme Server",
      "ConnectedStateCoalescingInterval": 2.0,
      "Port": 8080,
    ]

    try! webServer.start(options: options)
  }

  func handleFormatsRequest(request: GCDWebServerRequest)
    -> GCDWebServerResponse
  {
    var formats: [[String: Any]] = []
    for format in ARWorldTrackingConfiguration.supportedVideoFormats {
      let devicePosition: String
      switch format.captureDevicePosition {
      case AVCaptureDevice.Position.front: devicePosition = "front"
      case AVCaptureDevice.Position.back: devicePosition = "back"
      default: devicePosition = "unspecified"
      }

      formats.append([
        "devicePosition": devicePosition,
        "deviceType": format.captureDeviceType.rawValue,
        "framesPerSecond": format.framesPerSecond,
        "imageHeight": format.imageResolution.height,
        "imageWidth": format.imageResolution.width,
      ])
    }

    return GCDWebServerDataResponse.init(jsonObject: formats)!
  }

  func handleScanRequest(request: GCDWebServerRequest)
    -> GCDWebServerResponse?
  {
    scanLock.lock()

    if scanState != nil {
      scanLock.unlock()
      return GCDWebServerResponse(
        statusCode: GCDWebServerClientErrorHTTPStatusCode
          .httpStatusCode_Conflict.rawValue)
    }

    scanState = ScanState(
      fps: Double(request.query?["fps"] ?? "0")!,
      frameIndex: 0,
      numFrames: Int(request.query?["nframes"] ?? "1")!,
      start: TimeInterval(request.query?["start"] ?? "0")!
    )

    let format = Int(request.query?["format"] ?? "0")!
    session.activate(videoFormat: format)

    scanLock.unlock()

    let resp = GCDWebServerStreamedResponse.init(
      contentType: "text/plain",
      asyncStreamBlock: { block in
        block("this is a block ".data(using: .utf8), nil)
      })
    return resp
  }

  func onFrame(frame: ScanFrame) {
    scanLock.lock()
    if scanState == nil {
      scanLock.unlock()
      return
    }

    if scanState!.fps != 0
      && (Date().timeIntervalSince1970 - scanState!.start) * scanState!.fps
        <= Double(scanState!.frameIndex)
    {
      scanLock.unlock()
      return
    }

    ///

    scanLock.unlock()
  }
}
