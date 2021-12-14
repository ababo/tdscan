import ARKit

struct ScanFrame {
  public let time: TimeInterval
  public var image: CGImage?
  public let depthWidth: Int
  public let depthHeight: Int
  public var depths: [Float]
  public var depthConfidences: [UInt8]

  public static func decode(data: inout Data) -> ScanFrame {
    var offset = 0
    let time = data.withUnsafeBytes { ptr in
      (ptr.baseAddress! + offset).load(as: TimeInterval.self)
    }
    offset += MemoryLayout<TimeInterval>.size

    let imageWidth = data.withUnsafeBytes { ptr in
      (ptr.baseAddress! + offset).load(as: Int.self)
    }
    offset += MemoryLayout<Int>.size

    let imageHeight = data.withUnsafeBytes { ptr in
      (ptr.baseAddress! + offset).load(as: Int.self)
    }
    offset += MemoryLayout<Int>.size

    var image: CGImage? = nil
    if imageWidth > 0 && imageHeight > 0 {
      var imageContext: CGContext?
      data.withUnsafeMutableBytes { ptr in
        imageContext = CGContext(
          data: ptr.baseAddress! + offset, width: imageWidth,
          height: imageHeight,
          bitsPerComponent: 8,
          bytesPerRow: imageWidth * 4,
          space: CGColorSpaceCreateDeviceRGB(),
          bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)
      }
      offset += imageWidth * 4 * imageHeight
      image = imageContext!.makeImage()!
    }

    let depthWidth = data.withUnsafeBytes { ptr in
      (ptr.baseAddress! + offset).load(as: Int.self)
    }
    offset += MemoryLayout<Int>.size

    let depthHeight = data.withUnsafeBytes { ptr in
      (ptr.baseAddress! + offset).load(as: Int.self)
    }
    offset += MemoryLayout<Int>.size

    let depthCount = depthHeight * depthWidth

    var depths: [Float] = []
    depths.reserveCapacity(depthCount)
    data.withUnsafeBytes { ptr in
      for i in 0..<depthCount {
        depths.append(
          (ptr.baseAddress! + offset + i * MemoryLayout<Float>.stride).load(
            as: Float.self))
      }
    }
    offset += MemoryLayout<Float>.stride * depthCount

    let depthConfidences = [UInt8](
      data.subdata(in: offset..<offset + depthCount))
    assert(data.count == offset + depthCount)

    return ScanFrame(
      time: time,
      image: image,
      depthWidth: depthWidth,
      depthHeight: depthHeight,
      depths: depths,
      depthConfidences: depthConfidences
    )
  }

  public func encode() -> Data {
    var data = Data()

    var time = self.time
    data.append(
      Data(
        bytes: &time,
        count: MemoryLayout<TimeInterval>.size))

    var imageWidth = image?.width ?? 0
    data.append(
      Data(
        bytes: &imageWidth,
        count: MemoryLayout<Int>.size))
    var imageHeight = image?.height ?? 0
    data.append(
      Data(
        bytes: &imageHeight,
        count: MemoryLayout<Int>.size))

    if image != nil {
      let imageData = image!.dataProvider!.data! as Data
      data.append(imageData)
    }

    var depthWidth = self.depthWidth
    data.append(
      Data(
        bytes: &depthWidth,
        count: MemoryLayout<Int>.size))
    var depthHeight = self.depthHeight
    data.append(
      Data(
        bytes: &depthHeight,
        count: MemoryLayout<Int>.size))
    var depths = self.depths
    data.append(
      Data(
        bytes: &depths,
        count: depthHeight * depthWidth * MemoryLayout<Float>.stride))
    data.append(Data(depthConfidences))

    return data
  }
}

struct ScanMetadata {
  public let intrinsicMatrix: simd_float3x3
  public let intrinsicMatrixRefDims: CGSize
  public let inverseDistortionTable: [Float]

  public func angleOfView() -> Float {
    atan(intrinsicMatrix[2][0] / intrinsicMatrix[0][0]) * 2.0
  }
}

class ScanSession: NSObject, ARSessionDelegate {
  public let arSession = ARSession()
  public var onFrame: ((inout ScanFrame, ScanMetadata?) -> Void)?
  public var trueDepthMetadata = [ScanMetadata?](
    repeating: nil,
    count: ARFaceTrackingConfiguration.supportedVideoFormats.count)

  var useCount = 0
  var videoFormat = 0

  override init() {
    super.init()
    arSession.delegate = self
  }

  public func activate() {
    assert(useCount >= 0)
    if useCount == 0 {
      run(videoFormat: 0)
    }
    useCount += 1
  }

  public func activate(videoFormat: Int) {
    assert(useCount >= 0)
    run(videoFormat: videoFormat)
    useCount += 1
  }

  func run(videoFormat: Int) {
    self.videoFormat = videoFormat

    let trueDepthFormats = ARFaceTrackingConfiguration.supportedVideoFormats
    if videoFormat < trueDepthFormats.count {
      let config = ARFaceTrackingConfiguration()
      config.videoFormat = trueDepthFormats[videoFormat]
      arSession.run(config, options: [])
    } else {
      let lidarFormats = ARWorldTrackingConfiguration.supportedVideoFormats
      let config = ARWorldTrackingConfiguration()
      config.videoFormat = lidarFormats[videoFormat - trueDepthFormats.count]
      config.frameSemantics = .sceneDepth
      arSession.run(config)
    }
  }

  public func release() {
    assert(useCount >= 0)
    useCount -= 1
    if useCount == 0 {
      arSession.pause()
    }
  }

  func session(_ session: ARSession, didUpdate: ARFrame) {
    let context = CIContext(options: nil)
    let ciImage = CIImage(cvPixelBuffer: didUpdate.capturedImage)
    let cgImage = context.createCGImage(ciImage, from: ciImage.extent)!

    var depthMap: CVPixelBuffer? = nil
    var confidenceMap: CVPixelBuffer? = nil
    var quality = 0
    if didUpdate.capturedDepthData != nil {
      depthMap = didUpdate.capturedDepthData!.depthDataMap
      quality = didUpdate.capturedDepthData!.depthDataQuality == .high ? 3 : 1
    } else if didUpdate.sceneDepth != nil {
      depthMap = didUpdate.sceneDepth!.depthMap
      confidenceMap = didUpdate.sceneDepth!.confidenceMap!
    } else {
      return
    }

    let depthWidth = CVPixelBufferGetWidth(depthMap!)
    let depthHeight = CVPixelBufferGetHeight(depthMap!)

    var depths: [Float] = []
    var depthConfidences: [UInt8] = []
    depths.reserveCapacity(depthWidth * depthHeight)
    depthConfidences.reserveCapacity(depthWidth * depthHeight)

    let lockFlags = CVPixelBufferLockFlags(rawValue: 0)
    CVPixelBufferLockBaseAddress(depthMap!, lockFlags)
    let depthBuf = unsafeBitCast(
      CVPixelBufferGetBaseAddress(depthMap!),
      to: UnsafeMutablePointer<Float32>.self)

    if confidenceMap != nil {
      CVPixelBufferLockBaseAddress(confidenceMap!, lockFlags)
      let confidenceBuf = unsafeBitCast(
        CVPixelBufferGetBaseAddress(confidenceMap!),
        to: UnsafeMutablePointer<UInt8>.self)
      for i in 0..<depthWidth * depthHeight {
        depths.append(depthBuf[i])
        depthConfidences.append(confidenceBuf[i] + 1)
      }
      CVPixelBufferUnlockBaseAddress(confidenceMap!, lockFlags)
    } else {
      for i in 0..<depthWidth * depthHeight {
        depths.append(depthBuf[i])
        depthConfidences.append(UInt8(quality))
      }
    }

    CVPixelBufferUnlockBaseAddress(depthMap!, lockFlags)

    var frame = ScanFrame(
      time: didUpdate.timestamp,
      image: cgImage,
      depthWidth: depthWidth,
      depthHeight: depthHeight,
      depths: depths,
      depthConfidences: depthConfidences
    )

    var metadata: ScanMetadata?
    if self.videoFormat
      < ARFaceTrackingConfiguration.supportedVideoFormats.count
    {
      if trueDepthMetadata[self.videoFormat] == nil {
        let calibrationData = didUpdate.capturedDepthData!
          .cameraCalibrationData!
        trueDepthMetadata[self.videoFormat] = ScanMetadata(
          intrinsicMatrix: calibrationData.intrinsicMatrix,
          intrinsicMatrixRefDims: calibrationData
            .intrinsicMatrixReferenceDimensions,
          inverseDistortionTable: ScanSession.floatArrayFromData(
            data: calibrationData.inverseLensDistortionLookupTable!)
        )
      }
      metadata = trueDepthMetadata[self.videoFormat]
    }

    onFrame?(&frame, metadata)
  }

  static func floatArrayFromData(data: Data) -> [Float] {
    let count = data.count / 4
    var array = [Float](repeating: 0, count: count)
    (data as NSData).getBytes(&array, length: count * 4)
    return array
  }
}
