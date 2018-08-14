
extern crate cxdbg;
use cxdbg::DebugClient;
use cxdbg::proto::{Event, PageApi, NetworkApi};
use cxdbg::proto::Event::*;

extern crate rustyline;
use rustyline::error::ReadlineError;
use rustyline::Editor;

use std::thread;
use std::sync::{Arc, Mutex};

fn process_event(ev: &Event) {
    match ev {
        Inspector_detached { reason } => {
            println!("Inspector detached: {}", reason);
        }
        Inspector_targetCrashed => {
            println!("Inspector target has crashed");
        }
        Performance_metrics { .. } => (),
        Page_domContentEventFired { .. } => {
        }
        Page_loadEventFired { .. } => (),
        Page_lifecycleEvent { .. } => (),
        Page_frameAttached { frameId, parentFrameId, stack } => (),
        Page_frameNavigated { .. } => (),
        Page_frameDetached { frameId } => (),
        Page_frameStartedLoading { frameId } => (),
        Page_frameStoppedLoading { frameId } => (),
        Page_frameScheduledNavigation { .. } => (),
        Page_frameClearedScheduledNavigation { .. } => (),
        Page_frameResized => (),
        Page_javascriptDialogOpening { .. } => (),
        Page_javascriptDialogClosed { .. } => (),
        Page_screencastFrame { .. } => (),
        Page_screencastVisibilityChanged { visible } => (),
        Page_interstitialShown => (),
        Network_resourceChangedPriority { .. } => (),
        Network_requestWillBeSent { requestId, loaderId, documentURL, .. } => {
            println!("Request {}", documentURL);
        }
        Network_requestServedFromCache { .. } => (),
        Network_responseReceived { .. } => {
        }
        Network_dataReceived { .. } => (),
        Network_loadingFinished { .. } => {
        }
        Network_loadingFailed { .. } => {
        }
        Network_webSocketWillSendHandshakeRequest {..} => {
        }
        Network_webSocketHandshakeResponseReceived {..} => {
        }
        Network_webSocketCreated { .. } => {
        }
        Network_webSocketClosed { .. } => {
        }
        Network_webSocketFrameReceived { .. } => {
        }
        Network_webSocketFrameError { .. } => {
        }
        Network_webSocketFrameSent { .. } => {
        }
        Network_eventSourceMessageReceived { .. } => (),
        Network_requestIntercepted { .. } => (),
        Target_targetCreated { targetInfo } => {
        }
        Target_targetInfoChanged { targetInfo } => {
        }
        Target_targetDestroyed { targetId } => {
        }
        Target_attachedToTarget { sessionId, targetInfo, waitingForDebugger } => {
        }
        Target_detachedFromTarget { sessionId, targetId } => {
        }
        Target_receivedMessageFromTarget { .. } => {
        }
        _ => (),
    }
}

fn main() {
    let mut c = DebugClient::connect(9222);
    PageApi::enable(&mut c).expect("Could not enable Page events");
    NetworkApi::enable(&mut c, None, None).expect("Could not enable Network events");
    let data = Arc::new(Mutex::new(c));

    let thread_data = data.clone();
    thread::spawn(move ||
                  loop {
                      let mut c = thread_data.lock().unwrap();
                       c.poll().unwrap(); 
                       for ev in c.pending_events.drain(..) {
                           process_event(&ev);
                       }
                  });


    // `()` can be used when no completer is required
    let mut rl = Editor::<()>::new();
    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_ref());
                println!("Line: {}", line);
            },
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break
            },
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break
            },
            Err(err) => {
                println!("Error: {:?}", err);
                break
            }
        }
    }
}
