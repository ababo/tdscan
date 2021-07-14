import ARKit

class ScanSession: NSObject, ARSessionDelegate {
  public let arSession = ARSession()
  public var onFrame: (() -> Void)?

  var useCount = 0

  override init() {
    super.init()
    arSession.delegate = self
  }

  public func activate() {
    assert(useCount >= 0)
    if useCount == 0 {
      arSession.run(ARObjectScanningConfiguration())
    }
    useCount += 1
  }

  public func release() {
    assert(useCount >= 0)
    useCount -= 1
    if useCount == 0 {
      arSession.pause()
    }
  }

  func session(_ session: ARSession, didUpdate: ARFrame) {
    let image = UIImage(
      ciImage: CIImage(cvPixelBuffer: didUpdate.capturedImage))
    // print("png size", image.pngData()?.count) Too slow!!!
    onFrame?()
  }
}
