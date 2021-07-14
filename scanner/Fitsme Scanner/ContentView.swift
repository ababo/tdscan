import RealityKit
import SwiftUI

struct ContentView: View {
  struct ARViewContainer: UIViewRepresentable {
    let session: ScanSession

    func makeUIView(context: Context) -> ARView {
      let view = ARView(
        frame: .zero, cameraMode: .ar, automaticallyConfigureSession: false)
      view.session = session.arSession
      return view
    }

    func updateUIView(_ uiView: ARView, context: Context) {}
  }

  @State var session: ScanSession

  let viewTopOffset: CGFloat = 50
  let viewBottomOffset: CGFloat = 50

  var body: some View {
    Dimmer {
      ZStack {
        Color.black
          .ignoresSafeArea()
        VStack {
          Rectangle()
            .fill(Color.black)
            .frame(height: viewTopOffset)
          ARViewContainer(session: session)
            .onAppear {
              session.activate()
            }
            .onDisappear {
              session.release()
            }
          Rectangle()
            .fill(Color.black)
            .frame(height: viewBottomOffset)
        }
      }
    }
  }
}
