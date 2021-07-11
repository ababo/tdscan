import Foundation
import SwiftUI

struct Dimmer<Content: View>: View {
  let content: () -> Content

  let coverTimeout: TimeInterval = 30
  let dimTimeout: TimeInterval = 20

  @State var active = true
  @State var covered = false
  @State var lastTapped = Date()
  @State var normalBrightness = UIScreen.main.brightness

  let screenBounds = UIScreen.main.nativeBounds

  let timer = Timer.publish(every: 1, on: .main, in: .common).autoconnect()
  let willEnterForegroundPublisher = NotificationCenter.default.publisher(
    for: UIApplication.willEnterForegroundNotification)
  let willResignActivePublisher = NotificationCenter.default.publisher(
    for: UIApplication.willResignActiveNotification)

  var body: some View {
    Group {
      if covered {
        Rectangle()
          .fill(Color.black)
          .frame(
            width: screenBounds.width,
            height: screenBounds.height
          )
          .edgesIgnoringSafeArea(.all)
          .onTapGesture {
            restore()
          }
      } else {
        content()
          .onReceive(timer) { now in
            if !active {
              return
            }
            let elapsed = now.timeIntervalSince(lastTapped)
            if elapsed >= coverTimeout {
              covered = true
            } else if elapsed >= dimTimeout {
              UIScreen.main.brightness = 0
            }
          }
          .onTapGesture {
            restore()
          }
      }
    }.onReceive(
      willResignActivePublisher,
      perform: { output in
        active = false
        restore()
      }
    ).onReceive(
      willEnterForegroundPublisher,
      perform: { output in
        active = true
        restore()
      })
  }

  func restore() {
    UIScreen.main.brightness = normalBrightness
    lastTapped = Date()
    covered = false
  }
}
