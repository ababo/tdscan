import SwiftUI

struct ContentView: View {
  var body: some View {
    Dimmer {
      VStack {
        Spacer()
        Text("Main View")
        Spacer()
      }
    }
  }
}

#if DEBUG
  struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
      ContentView()
    }
  }
#endif
