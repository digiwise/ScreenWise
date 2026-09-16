use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, oneshot};

/// Test that reproduces a historically observed "channel lagged" error
/// (`error receiving audio data: channel lagged by 214`).
///
/// The broadcast channel with capacity 1000 returns RecvError::Lagged when
/// the receiver falls behind. Currently this causes the entire recording to fail.
#[tokio::test]
async fn test_broadcast_channel_lag_causes_error() {
    // Create a broadcast channel with small capacity to trigger lag quickly
    let (tx, mut rx) = broadcast::channel::<Vec<f32>>(10);

    // Simulate fast producer (audio input) - send more messages than buffer can hold
    for i in 0..20 {
        let chunk = vec![i as f32; 100];
        let _ = tx.send(chunk);
    }

    // Now try to receive - this should return Lagged error
    let result = rx.recv().await;

    match result {
        Err(broadcast::error::RecvError::Lagged(n)) => {
            println!("Received expected Lagged error: {} messages skipped", n);
            // This is the current behavior that causes the recording to restart
            assert!(n > 0, "Should have lagged by at least 1 message");
        }
        Ok(_) => {
            panic!("Expected Lagged error but got Ok");
        }
        Err(broadcast::error::RecvError::Closed) => {
            panic!("Expected Lagged error but channel was closed");
        }
    }
}

/// Test that demonstrates the fix: handle Lagged error gracefully
/// Instead of terminating, we should continue receiving after lag
#[tokio::test]
async fn test_broadcast_channel_lag_recovery() {
    let (tx, mut rx) = broadcast::channel::<Vec<f32>>(10);
    let received_count = Arc::new(AtomicU32::new(0));
    let lag_count = Arc::new(AtomicU32::new(0));

    // Send 25 messages (more than buffer size of 10)
    for i in 0..25 {
        let chunk = vec![i as f32; 100];
        let _ = tx.send(chunk);
    }

    // Try to receive with graceful lag handling
    loop {
        match rx.recv().await {
            Ok(_chunk) => {
                received_count.fetch_add(1, Ordering::Relaxed);
                // Successfully received a chunk after potential lag
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                lag_count.fetch_add(1, Ordering::Relaxed);
                println!("Lagged by {} messages, continuing...", n);
                // Key fix: continue instead of returning error
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => {
                break;
            }
        }

        // Stop after receiving some messages
        if received_count.load(Ordering::Relaxed) >= 5 {
            break;
        }
    }

    let received = received_count.load(Ordering::Relaxed);
    let lagged = lag_count.load(Ordering::Relaxed);

    println!(
        "Received {} messages, experienced {} lag events",
        received, lagged
    );

    // With graceful handling, we should have received some messages despite lag
    assert!(received > 0, "Should have received at least some messages");
    assert!(lagged > 0, "Should have experienced at least one lag event");
}

/// Test simulating real audio recording scenario with slow consumer
#[tokio::test]
async fn test_slow_consumer_causes_lag() {
    let (tx, mut rx) = broadcast::channel::<Vec<f32>>(100);
    let (consumer_started_tx, consumer_started_rx) = oneshot::channel();
    let (backlog_ready_tx, backlog_ready_rx) = oneshot::channel();

    // Producer: simulates fast audio input (~44100 samples/sec in chunks)
    let producer = tokio::spawn(async move {
        tx.send(vec![0.0f32; 1024]).unwrap();
        consumer_started_rx.await.unwrap();
        for _i in 0..200 {
            let chunk = vec![0.0f32; 1024]; // ~23ms of audio at 44.1kHz
            tx.send(chunk).unwrap();
        }
        backlog_ready_tx.send(()).unwrap();
    });

    // Consumer: simulates slow transcription processing
    let consumer = tokio::spawn(async move {
        // Hold the consumer while the producer builds a known backlog. Wall-clock
        // sleeps do not guarantee lag on Windows or a busy CI worker.
        rx.recv().await.unwrap();
        consumer_started_tx.send(()).unwrap();
        backlog_ready_rx.await.unwrap();
        let mut received = 0;
        let mut lagged = 0;
        loop {
            match rx.recv().await {
                Ok(_chunk) => {
                    received += 1;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    println!("Consumer lagged by {} chunks at message {}", n, received);
                    lagged += n;
                    // With fix: continue instead of failing
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }

            if received >= 10 {
                break;
            }
        }
        (received, lagged)
    });

    producer.await.unwrap();
    let (received, lagged) = consumer.await.unwrap();

    println!("Consumer received {} chunks", received);
    assert!(lagged > 0, "Slow consumer should have experienced lag");
    assert!(received > 0, "Should have received some chunks despite lag");
}
