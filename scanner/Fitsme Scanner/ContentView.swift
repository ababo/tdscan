import ARKit
import RealityKit
import SwiftUI

struct ContentView: View {
  @State var viewContainer = ARViewContainer()

  var body: some View {
    Dimmer {
      viewContainer
        .edgesIgnoringSafeArea(.all)
        .onAppear {
          viewContainer.activate()
        }
        .onDisappear {
          viewContainer.release()
        }
    }
  }
}

struct ARViewContainer: UIViewRepresentable {
  let session = ARSession()
  var useCount = 0

  public mutating func activate() {
    if useCount == 0 {
      session.run(ARObjectScanningConfiguration())
    }
    useCount += 1
  }

  public mutating func release() {
    useCount -= 1
    if useCount == 0 {
      session.pause()
    }
  }

  func makeUIView(context: Context) -> ARView {
    let view = ARView(
      frame: .zero, cameraMode: .ar, automaticallyConfigureSession: false)
    view.session = session
    return view
  }

  func updateUIView(_ uiView: ARView, context: Context) {}
}

#if DEBUG
  struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
      ContentView()
    }
  }
#endif
