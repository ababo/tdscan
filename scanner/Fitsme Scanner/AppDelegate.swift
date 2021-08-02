import ARKit
import SwiftUI
import UIKit

@main
class AppDelegate: UIResponder, UIApplicationDelegate {
  class Scan {
    public let campos: FmPoint3
    public let camvel: Float
    public let fps: Double
    public let name: String
    public let nframes: Int
    public let start: TimeInterval
    public let viewel: Float

    public var inFrameIndex = 0
    public var outFrameIndex = 0
    public var writer: FmWriter?
    public var block: GCDWebServerBodyReaderCompletionBlock?
    public var lastOutUptime: TimeInterval = 0

    public init(
      campos: FmPoint3, camvel: Float, fps: Double, name: String, nframes: Int,
      start: TimeInterval, viewel: Float
    ) {
      self.campos = campos
      self.camvel = camvel
      self.fps = fps
      self.name = name
      self.nframes = nframes
      self.start = start
      self.viewel = viewel
    }

    public func nextOutFrameReady() -> Bool { inFrameIndex > outFrameIndex }
    public func noMoreInFrames() -> Bool { inFrameIndex == nframes }
    public func noMoreOutFrames() -> Bool { outFrameIndex == nframes }
  }

  let lock = NSLock()
  let session = ScanSession()
  let webServer = GCDWebServer()
  let outQueue = DispatchQueue(label: "out")

  var scan: Scan?
  var window: UIWindow?

  func getScan() -> Scan? {
    var scan: Scan?
    lock.lock()
    scan = self.scan
    lock.unlock()
    return scan
  }

  func setScan(scan: Scan?) {
    lock.lock()
    self.scan = scan
    lock.unlock()
  }

  func setScanIfNone(scan: Scan) -> Bool {
    var set = false
    lock.lock()
    if self.scan == nil {
      self.scan = scan
      set = true
    }
    lock.unlock()
    return set
  }

  override init() {
    super.init()
    session.onFrame = onFrame
  }

  func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication
      .LaunchOptionsKey: Any]?
  ) -> Bool {
    application.isIdleTimerDisabled = true
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
    GCDWebServer.setLogLevel(3)  // Warning.

    webServer.addHandler(
      forMethod: "GET", path: "/formats", request: GCDWebServerRequest.self,
      processBlock: onFormatsRequest)

    webServer.addHandler(
      forMethod: "GET", path: "/scan",
      request: GCDWebServerRequest.self,
      processBlock: onScanRequest)

    let options: [String: Any] = [
      "AutomaticallySuspendInBackground": false,
      "BindToLocalhost": false,
      "BonjourName": "Fitsme Server",
      "ConnectedStateCoalescingInterval": 2.0,
      "Port": 9321,
      "ServerName": "Fitsme Scanner",
    ]

    try! webServer.start(options: options)
  }

  func onFormatsRequest(request: GCDWebServerRequest)
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

  func onScanRequest(request: GCDWebServerRequest)
    -> GCDWebServerResponse?
  {
    print(
      "Processing '/scan' request with query "
        + (request.query?.description ?? ""))

    let uts = Date().timeIntervalSince1970
    let uptime = ProcessInfo.processInfo.systemUptime

    if let scan = getScan() {
      if uptime - scan.lastOutUptime > 5 {
        finishScan(output: false)
        print("Finished previously aborted '/scan' request")
      }
    }

    let campos = request.query?["campos"]?.split(separator: ",")
      .map(Float.init).compactMap { $0 }
    let camvel = Float(request.query?["camvel"] ?? "")
    let fmt = UInt(request.query?["fmt"] ?? "0")
    let fps = Double(request.query?["fps"] ?? "0")
    let name = request.query?["name"] ?? "\(UIDevice.current.name)-\(uts)"
    let nframes = UInt(request.query?["nframes"] ?? "1")
    let start = TimeInterval(request.query?["start"] ?? String(uts))
    let viewel = Float(request.query?["viewel"] ?? "0")

    let numFormats = ARWorldTrackingConfiguration.supportedVideoFormats.count
    if campos == nil || campos!.count != 3 || camvel == nil || fmt == nil
      || fmt! >= numFormats || fps == nil || fps! < 0 || nframes == nil
      || start == nil || start! < uts || viewel == nil
    {
      print("Bad '/scan' request arguments")
      return GCDWebServerResponse(
        statusCode: GCDWebServerClientErrorHTTPStatusCode
          .httpStatusCode_BadRequest.rawValue)
    }

    let scan = Scan(
      campos: FmPoint3(x: campos![0], y: campos![1], z: campos![2]),
      camvel: camvel!, fps: fps!, name: name, nframes: Int(nframes!),
      start: start! - uts + uptime, viewel: viewel!)

    if !setScanIfNone(scan: scan) {
      print("Refused '/scan' request, busy handling previous request")
      return GCDWebServerResponse(
        statusCode: GCDWebServerClientErrorHTTPStatusCode
          .httpStatusCode_Conflict.rawValue)
    }

    session.activate(videoFormat: Int(fmt!))
    AudioServicesPlaySystemSound(1113)

    return GCDWebServerStreamedResponse(
      contentType: "text/plain",
      asyncStreamBlock: { block in
        self.outQueue.sync { self.onStreamBlock(block: block) }
      })
  }

  func onStreamBlock(block: @escaping GCDWebServerBodyReaderCompletionBlock) {
    var scan: Scan?
    while true {
      scan = getScan()
      if scan == nil || scan!.nextOutFrameReady() || scan!.noMoreOutFrames() {
        break
      }
      Thread.sleep(forTimeInterval: 0.01)
    }

    if scan == nil {
      block(Data(), nil)
      return
    }

    scan!.lastOutUptime = ProcessInfo.processInfo.systemUptime

    if scan!.noMoreOutFrames() {
      finishScan(output: true)
      print("Finished '/scan' request")
      return
    }

    let url = AppDelegate.frameURL(index: scan!.outFrameIndex)
    var data = try! Data(contentsOf: url)
    try! FileManager.default.removeItem(at: url)
    let frame = ScanFrame.decode(data: &data)

    if scan!.writer == nil {
      scan!.writer = createWriter(block: block, scan: scan!, frame: frame)
    }

    let png = [UInt8](UIImage(cgImage: frame.image).pngData()!)
    png.withUnsafeBufferPointer { pngPtr in
      frame.depths.withUnsafeBufferPointer { depthsPtr in
        frame.depthConfidences.withUnsafeBufferPointer { depthConfidencesPtr in
          scan!.name.cString(using: .utf8)!.withUnsafeBufferPointer { namePtr in
            let image = FmImage(
              type: kFmImagePng, data: pngPtr.baseAddress, data_size: png.count)
            var fmFrame = FmScanFrame(
              scan: namePtr.baseAddress,
              time: Int64((frame.time - scan!.start) * 1_000_000_000),
              image: image, depths: depthsPtr.baseAddress,
              depths_size: frame.depths.count,
              depth_confidences: depthConfidencesPtr.baseAddress,
              depth_confidences_size: frame.depthConfidences.count)
            let err = fm_write_scan_frame(scan!.writer, &fmFrame)
            assert(err == kFmOk)
          }
        }
      }
    }

    print("Sent frame \(scan!.outFrameIndex)")
    scan!.outFrameIndex += 1
    setScan(scan: scan)
  }

  func finishScan(output: Bool) {
    let scan = getScan()!

    if !output {
      scan.block = nil
    }
    setScan(scan: scan)
    let err = fm_close_writer(scan.writer)
    assert(err == kFmOk)

    setScan(scan: nil)
  }

  func createWriter(
    block: @escaping GCDWebServerBodyReaderCompletionBlock, scan: Scan,
    frame: ScanFrame
  ) -> FmWriter {
    var writer: FmWriter?

    scan.block = block
    let scanPtr = UnsafeMutableRawPointer(
      Unmanaged.passUnretained(scan).toOpaque())
    var err = fm_create_writer(onWriterCallback, scanPtr, &writer)
    assert(err == kFmOk)

    scan.name.cString(using: .utf8)!.withUnsafeBufferPointer { namePtr in
      var fmScan = FmScan(
        name: namePtr.baseAddress, camera_position: scan.campos,
        camera_velocity: scan.camvel, view_elevation: scan.viewel,
        image_width: Int32(frame.image.width),
        image_height: Int32(frame.image.height),
        depth_width: Int32(frame.depthWidth),
        depth_height: Int32(frame.depthHeight)
      )
      err = fm_write_scan(writer, &fmScan)
      assert(err == kFmOk)
    }

    return writer!
  }

  let onWriterCallback:
    @convention(c) (UnsafePointer<UInt8>?, Int, UnsafeMutableRawPointer?) ->
      FmError = { (fm_data, fm_size, cb_data) in
        let scan = Unmanaged<Scan>.fromOpaque(cb_data!).takeUnretainedValue()
        scan.block?(Data(bytes: fm_data!, count: fm_size), nil)
        return kFmOk
      }

  func onFrame(frame: ScanFrame) {
    let scan = getScan()
    if scan == nil || scan!.noMoreInFrames() {
      return
    }

    if scan!.fps != 0
      && (frame.time - scan!.start) * scan!.fps <= Double(scan!.inFrameIndex)
    {
      return
    }

    let url = AppDelegate.frameURL(index: scan!.inFrameIndex)
    try! frame.encode().write(to: url)
    scan!.inFrameIndex += 1
    setScan(scan: scan)

    if scan!.inFrameIndex == scan!.nframes {
      AudioServicesPlaySystemSound(1114)
      session.release()
    }
  }

  static func frameURL(index: Int) -> URL {
    let tempDir = FileManager.default.temporaryDirectory
    let fileName = "frame\(index).bin"
    return tempDir.appendingPathComponent(fileName)
  }
}
