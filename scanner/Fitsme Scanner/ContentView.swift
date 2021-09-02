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
          ZStack {
            ARViewContainer(session: session)
              .onAppear {
                session.activate()
              }
              .onDisappear {
                session.release()
              }
            GeometryReader { geometry in
              Path { path in
                let size = geometry.size
                path.move(to: CGPoint(x: 0, y: size.height / 2))
                path.addLine(to: CGPoint(x: size.width, y: size.height / 2))
                path.move(to: CGPoint(x: size.width / 2, y: 0))
                path.addLine(to: CGPoint(x: size.width / 2, y: size.height))
              }.stroke(Color.red)
            }
          }
          Rectangle()
            .fill(Color.black)
            .frame(height: viewBottomOffset)
        }
      }
    }
  }
}
