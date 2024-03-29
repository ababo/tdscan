import ARKit
import SwiftUI
import UIKit

extension Double: _VectorMath {}
extension SIMD4: _VectorMath {}

protocol UndistortElement {
  associatedtype Unpacked: _VectorMath
  func unpack() -> Unpacked
  static func pack(value: Unpacked) -> Self
}

extension Float: UndistortElement {
  typealias Unpacked = Double
  func unpack() -> Unpacked { Double(self) }
  static func pack(value: Unpacked) -> Self {
    // Reduce size of fm-file by limiting precision to 0.1mm.
    Float((value * 10000.0).rounded() / 10000.0)
  }
}

struct RGBA {
  var r: UInt8
  var g: UInt8
  var b: UInt8
  var a: UInt8
}

extension RGBA: UndistortElement {
  typealias Unpacked = SIMD4<Double>
  func unpack() -> Unpacked {
    SIMD4<Double>(
      Double(self.r),
      Double(self.g),
      Double(self.b),
      Double(self.a))
  }
  static func pack(value: Unpacked) -> Self {
    RGBA(
      r: UInt8(value[0]),
      g: UInt8(value[1]),
      b: UInt8(value[2]),
      a: UInt8(value[3]))
  }
}

@main
class AppDelegate: UIResponder, UIApplicationDelegate {
  class Scan {
    public let eye: FmPoint3
    public let ctr: FmPoint3
    public let cua: Float
    public let vel: Float
    public let fps: Double
    public let name: String
    public let nof: Int
    public let at: TimeInterval
    public let imgrt: Int
    public let undist: Bool
    public let trueDepth: Bool

    public var inFrameIndex = 0
    public var outFrameIndex = 0
    public var writer: FmWriter?
    public var block: GCDWebServerBodyReaderCompletionBlock?
    public var lastOutUptime: TimeInterval = 0
    public var metadata: ScanMetadata?
    public var undistortImageMap: ([Double], [Double])?
    public var undistortDepthMap: ([Double], [Double])?

    public init(
      eye: FmPoint3, ctr: FmPoint3, cua: Float, vel: Float, fps: Double,
      name: String, nof: Int, at: TimeInterval, imgrt: Int, undist: Bool,
      trueDepth: Bool
    ) {
      self.eye = eye
      self.ctr = ctr
      self.cua = cua
      self.vel = vel
      self.fps = fps
      self.name = name
      self.nof = nof
      self.at = at
      self.imgrt = imgrt
      self.undist = undist
      self.trueDepth = trueDepth
    }

    public func nextOutFrameReady() -> Bool { inFrameIndex > outFrameIndex }
    public func noMoreInFrames() -> Bool { inFrameIndex == nof }
    public func noMoreOutFrames() -> Bool { outFrameIndex == nof }
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
    UIDevice.current.beginGeneratingDeviceOrientationNotifications()
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
      "Port": 9321,
      "ServerName": "Fitsme Scanner",
    ]

    try! webServer.start(options: options)
  }

  func onFormatsRequest(request: GCDWebServerRequest)
    -> GCDWebServerResponse
  {
    var formats: [[String: Any]] = []
    for format in ARFaceTrackingConfiguration.supportedVideoFormats
      + ARWorldTrackingConfiguration.supportedVideoFormats
    {
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

    let upAngle: Float
    switch UIDevice.current.orientation {
    case .portrait:
      upAngle = -1.57079632679
    case .portraitUpsideDown:
      upAngle = 1.57079632679
    case .landscapeLeft:
      upAngle = 0
    case .landscapeRight:
      upAngle = 3.1415926536
    default:
      print("Failed to determine camera up angle")
      upAngle = Float.nan
    }

    let eye = request.query?["y"]?.split(separator: ",")
      .map(Float.init).compactMap { $0 }
    let ctr =
      request.query?["c"]?.split(separator: ",")
      .map(Float.init).compactMap { $0 } ?? [0.0, 0.0, 0.0]
    let cua = Float(request.query?["cua"] ?? String(upAngle))
    let vel = Float(request.query?["vel"] ?? "")
    let fmt = UInt(request.query?["fmt"] ?? "0")
    let fps = Double(request.query?["fps"] ?? "0")
    let name = request.query?["name"] ?? UUID().uuidString
    let nof = UInt(request.query?["nof"] ?? "1")
    let at = TimeInterval(request.query?["at"] ?? String(uts))
    let imgrt = UInt(request.query?["imgrt"] ?? "1")
    let undist = Bool(request.query?["undist"] ?? "false")

    let numFrontFmts = ARFaceTrackingConfiguration.supportedVideoFormats.count
    let numBackFmts = ARWorldTrackingConfiguration.supportedVideoFormats.count
    if eye == nil || eye!.count != 3 || ctr.count != 3 || cua == nil
      || vel == nil || fmt == nil || fmt! >= numFrontFmts + numBackFmts
      || fps == nil || fps! < 0 || nof == nil || at == nil || at! < uts
      || imgrt == nil || undist == nil
    {
      print("Bad '/scan' request arguments")
      return GCDWebServerResponse(
        statusCode: GCDWebServerClientErrorHTTPStatusCode
          .httpStatusCode_BadRequest.rawValue)
    }

    let scan = Scan(
      eye: FmPoint3(x: eye![0], y: eye![1], z: eye![2]),
      ctr: FmPoint3(x: ctr[0], y: ctr[1], z: ctr[2]),
      cua: cua!, vel: vel!, fps: fps!, name: name, nof: Int(nof!),
      at: at! - uts + uptime, imgrt: Int(imgrt!),
      undist: undist!, trueDepth: fmt! < numFrontFmts)

    if !setScanIfNone(scan: scan) {
      print("Refused '/scan' request, busy handling previous request")
      return GCDWebServerResponse(
        statusCode: GCDWebServerClientErrorHTTPStatusCode
          .httpStatusCode_Conflict.rawValue)
    }

    session.activate(videoFormat: Int(fmt!))
    AudioServicesPlaySystemSound(1113)

    return GCDWebServerStreamedResponse(
      contentType: "application/octet-stream",
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
    var frame = ScanFrame.decode(data: &data)

    if scan!.writer == nil {
      scan!.writer = createWriter(block: block, scan: scan!, frame: frame)
    }

    var png: [UInt8] = []
    if frame.image != nil {
      var img: UIImage
      if scan!.trueDepth {
        if scan!.undist {
          undistortImage(frame: &frame, scan: &scan!)
        }

        let orientation: UIImage.Orientation =
          UIDevice.current.orientation.isPortrait
          ? .downMirrored : .upMirrored
        img = UIImage(
          cgImage: frame.image!, scale: 1.0, orientation: orientation)
        UIGraphicsBeginImageContextWithOptions(img.size, true, img.scale)
        defer { UIGraphicsEndImageContext() }
        img.draw(in: CGRect(origin: .zero, size: img.size))
        img = UIGraphicsGetImageFromCurrentImageContext()!
      } else {
        img = UIImage(cgImage: frame.image!)
      }
      png = [UInt8](img.pngData()!)
    }

    if scan!.trueDepth {
      if scan!.undist {
        undistortDepths(frame: &frame, scan: &scan!)
      }

      if UIDevice.current.orientation.isPortrait {
        var tmp = [Float32](repeating: 0, count: frame.depthWidth)
        for i in 0..<frame.depthHeight / 2 {
          let j = frame.depthHeight - i - 1
          frame.depths.withUnsafeMutableBufferPointer { depthPtr in
            tmp.withUnsafeMutableBufferPointer { tmpPtr in
              memcpy(
                tmpPtr.baseAddress,
                depthPtr.baseAddress! + i * frame.depthWidth,
                frame.depthWidth * 4)
              memcpy(
                depthPtr.baseAddress! + i * frame.depthWidth,
                depthPtr.baseAddress! + j * frame.depthWidth,
                frame.depthWidth * 4)
              memcpy(
                depthPtr.baseAddress! + j * frame.depthWidth,
                tmpPtr.baseAddress,
                frame.depthWidth * 4)
            }
          }
        }
      } else {
        for i in 0..<frame.depthHeight {
          for j in 0..<frame.depthWidth / 2 {
            let base = i * frame.depthWidth
            let k = frame.depthWidth - j - 1
            frame.depths.swapAt(base + j, base + k)
          }
        }
      }
    }

    png.withUnsafeBufferPointer { pngPtr in
      frame.depths.withUnsafeBufferPointer { depthsPtr in
        frame.depthConfidences.withUnsafeBufferPointer { depthConfidencesPtr in
          scan!.name.cString(using: .utf8)!.withUnsafeBufferPointer { namePtr in
            var fmFrame = FmScanFrame(
              scan: namePtr.baseAddress,
              time: Int64((frame.time - scan!.at) * 1_000_000_000), image: nil,
              depths: depthsPtr.baseAddress,
              depths_size: frame.depths.count,
              depth_confidences: depthConfidencesPtr.baseAddress,
              depth_confidences_size: frame.depthConfidences.count)
            if frame.image != nil {
              fmFrame.image = UnsafeMutablePointer<FmImage>.allocate(
                capacity: 1)
              fmFrame.image[0].type = kFmImagePng
              fmFrame.image[0].data = pngPtr.baseAddress
              fmFrame.image[0].data_size = png.count
            }
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
    if scan.writer != nil {
      let err = fm_close_writer(scan.writer)
      assert(err == kFmOk)
    }

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

    var angleOfView = scan.metadata?.angleOfView() ?? Float.nan
    if angleOfView.isNaN {
      switch session.arSession.configuration!.videoFormat.captureDeviceType {
      case AVCaptureDevice.DeviceType.builtInWideAngleCamera:
        // See https://photo.stackexchange.com/questions/106509.
        angleOfView = 1.17460658659
      case AVCaptureDevice.DeviceType.builtInUltraWideCamera:
        // Ultra wide camera is claimed to have 120 degrees FoV.
        angleOfView = 2.09439510239
      default:
        fatalError("Unknown angle of view")
      }
    }

    scan.name.cString(using: .utf8)!.withUnsafeBufferPointer { namePtr in
      var fmScan = FmScan(
        name: namePtr.baseAddress,
        camera_angle_of_view: angleOfView,
        camera_up_angle: scan.cua,
        camera_angular_velocity: scan.vel,
        camera_initial_position: scan.eye,
        camera_initial_direction: scan.ctr,
        image_width: Int32(frame.image!.width),
        image_height: Int32(frame.image!.height),
        depth_width: Int32(frame.depthWidth),
        depth_height: Int32(frame.depthHeight),
        sensor_plane_depth: 1
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

  func onFrame(frame: inout ScanFrame, metadata: ScanMetadata?) {
    let scan = getScan()
    if scan == nil || scan!.noMoreInFrames() {
      return
    }

    if scan!.fps != 0
      && (frame.time - scan!.at) * scan!.fps <= Double(scan!.inFrameIndex)
    {
      return
    }

    if scan!.metadata == nil {
      scan!.metadata = metadata
    }

    // First frame must contain an image to be used when creating FmScan.
    if scan!.inFrameIndex % scan!.imgrt != 0 {
      frame.image = nil
    }

    let url = AppDelegate.frameURL(index: scan!.inFrameIndex)
    try! frame.encode().write(to: url)
    scan!.inFrameIndex += 1
    setScan(scan: scan)

    if scan!.inFrameIndex == scan!.nof {
      AudioServicesPlaySystemSound(1114)
      session.release()
    }
  }

  static func frameURL(index: Int) -> URL {
    let tempDir = FileManager.default.temporaryDirectory
    let fileName = "frame\(index).bin"
    return tempDir.appendingPathComponent(fileName)
  }

  func undistortImage(frame: inout ScanFrame, scan: inout Scan) {
    if scan.undistortImageMap == nil {
      scan.undistortImageMap = AppDelegate.buildUndistortMap(
        width: frame.image!.width, height: frame.image!.height,
        metadata: scan.metadata!)
    }

    let width = frame.image!.width
    let height = frame.image!.height
    let numPixels = width * height

    let srcData = frame.image!.dataProvider!.data!
    let srcBytePtr = CFDataGetBytePtr(srcData)
    let srcRawPtr = UnsafeRawPointer(srcBytePtr)!
    let srcRGBAPtr = srcRawPtr.bindMemory(to: RGBA.self, capacity: numPixels)
    let src = UnsafeBufferPointer<RGBA>(start: srcRGBAPtr, count: numPixels)

    let dstData = CFDataCreateMutableCopy(
      kCFAllocatorDefault, numPixels * 4, srcData)!
    let dstBytePtr = CFDataGetMutableBytePtr(dstData)
    let dstRawPtr = UnsafeMutableRawPointer(dstBytePtr)!
    let dstRGBAPtr = dstRawPtr.bindMemory(to: RGBA.self, capacity: numPixels)
    let dst = UnsafeMutableBufferPointer<RGBA>(
      start: dstRGBAPtr, count: numPixels)

    AppDelegate.undistort(
      dst: dst, src: src, width: frame.image!.width,
      height: frame.image!.height, undistortMap: scan.undistortImageMap!)

    let provider = CGDataProvider(data: dstData)!
    frame.image = CGImage(
      width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32,
      bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(),
      bitmapInfo: frame.image!.bitmapInfo, provider: provider, decode: nil,
      shouldInterpolate: false, intent: frame.image!.renderingIntent)
  }

  func undistortDepths(frame: inout ScanFrame, scan: inout Scan) {
    if scan.undistortDepthMap == nil {
      scan.undistortDepthMap = AppDelegate.buildUndistortMap(
        width: frame.depthWidth, height: frame.depthHeight,
        metadata: scan.metadata!)
    }

    var undistortedDepths = [Float](
      repeating: 0, count: frame.depthWidth * frame.depthHeight)

    undistortedDepths.withUnsafeMutableBufferPointer { dst in
      frame.depths.withUnsafeBufferPointer { src in
        AppDelegate.undistort(
          dst: dst, src: src, width: frame.depthWidth,
          height: frame.depthHeight, undistortMap: scan.undistortDepthMap!)
      }
    }

    frame.depths = undistortedDepths
  }

  static func undistort<T: UndistortElement>(
    dst: UnsafeMutableBufferPointer<T>, src: UnsafeBufferPointer<T>,
    width: Int, height: Int, undistortMap: ([Double], [Double])
  ) {
    func distortedDepth(i: Double, j: Double) -> T.Unpacked {
      var (i, j) = (Int(i), Int(j))
      if i >= height {
        i = height - 1
      }
      if j >= width {
        j = width - 1
      }
      return src.baseAddress![i * width + j].unpack()
    }

    for i in 0..<height {
      for j in 0..<width {
        let off = i * width + j

        let x = undistortMap.0[off]
        let y = undistortMap.1[off]

        let (x1, x2) = (floor(x), ceil(x))
        let (y1, y2) = (floor(y), ceil(y))

        let q11 = distortedDepth(i: y1, j: x1)
        let q12 = distortedDepth(i: y2, j: x1)
        let q21 = distortedDepth(i: y1, j: x2)
        let q22 = distortedDepth(i: y2, j: x2)

        var val = q11 * (x2 - x) * (y2 - y)
        val += q21 * (x - x1) * (y2 - y)
        val += q12 * (x2 - x) * (y - y1)
        val += q22 * (x - x1) * (y - y1)
        dst.baseAddress![off] = T.pack(value: val)
      }
    }
  }

  static func buildUndistortMap(
    width: Int, height: Int, metadata: ScanMetadata
  ) -> (
    [Double], [Double]
  ) {
    var intrinsicMatrix = [Double](repeating: 0, count: 9)
    for i in 0..<3 {
      for j in 0..<3 {
        intrinsicMatrix[i * 3 + j] = Double(metadata.intrinsicMatrix[j, i])
      }
    }

    let pixelScale =
      Double(width) / Double(metadata.intrinsicMatrixRefDims.width)
    intrinsicMatrix[0] *= pixelScale
    intrinsicMatrix[2] *= pixelScale
    intrinsicMatrix[4] *= pixelScale
    intrinsicMatrix[5] *= pixelScale

    let center = (intrinsicMatrix[2], intrinsicMatrix[5])

    let numPixels = width * height
    var xy = [Double](repeating: 0, count: numPixels * 2)
    var scale = [Double](repeating: 0, count: numPixels)

    var maxRadius: Double = 0
    for i in 0..<height {
      for j in 0..<width {
        let off = i * width + j
        let base = off * 2
        xy[base] = Double(j) - center.0
        xy[base + 1] = Double(i) - center.1
        scale[off] = sqrt(xy[base] * xy[base] + xy[base + 1] * xy[base + 1])
        if scale[off] > maxRadius {
          maxRadius = scale[off]
        }
      }
    }

    let idt = metadata.inverseDistortionTable
    for i in 0..<scale.count {
      let x = scale[i] / maxRadius * Double(idt.count)
      if Int(x) < idt.count - 1 {
        let lower = Double(idt[Int(x)])
        let upper = Double(idt[Int(x) + 1])
        scale[i] = (x - floor(x)) * (upper - lower) + lower + 1.0
      } else {
        scale[i] = Double(idt.last!) + 1.0
      }
    }

    var undistortMapX = [Double](repeating: 0, count: numPixels)
    var undistortMapY = [Double](repeating: 0, count: numPixels)
    for i in 0..<scale.count {
      undistortMapX[i] = scale[i] * xy[i * 2] + center.0
      undistortMapY[i] = scale[i] * xy[i * 2 + 1] + center.1
    }

    return (undistortMapX, undistortMapY)
  }
}
