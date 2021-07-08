//
//  ScannerApp.swift
//  Fitsme Scanner
//
//  Created by Simon Prykhodko on 03/07/2021.
//

import SwiftUI

@main
struct ScannerApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) var appDelegate

    var body: some Scene {
        return WindowGroup {
            ContentView()
        }
    }
}

class AppDelegate: NSObject, UIApplicationDelegate {
    static let webServer = GCDWebServer()
    
    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey : Any]? = nil) -> Bool {

        AppDelegate.webServer.addDefaultHandler(
            forMethod: "GET",
            request: GCDWebServerRequest.self,
            processBlock: { request in
                return GCDWebServerDataResponse(
                    html: "<html><body><p>Hello World</p></body></html>")
            })

        let options:[String : Any] = [
            "AutomaticallySuspendInBackground" : false,
            "BindToLocalhost": false,
            "BonjourName": "Fitsme Server",
            "ConnectedStateCoalescingInterval": 2.0,
            "Port": 8080,
        ]
        
        try! AppDelegate.webServer.start(options: options)
        
        return true
    }
}
