use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::Message;
use simd_json::base::{ValueAsContainer, ValueAsScalar};
use songbird::id::{ChannelId, GuildId, UserId};
use songbird::input::{File, Input};
use songbird::{Call, ConnectionInfo};
use std::num::NonZeroU64;
use tokio::signal;
use tokio::sync::mpsc;

struct VoiceServerUpdate {
    value: String,
    field: String,
}

fn vc_update(value: String, field: String) -> VoiceServerUpdate {
    VoiceServerUpdate { value, field }
}

fn data_token(value: String) -> VoiceServerUpdate {
    return vc_update(value, "token".to_owned());
}

fn data_session(value: String) -> VoiceServerUpdate {
    return vc_update(value, "session_id".to_owned());
}

fn data_endpoint(value: String) -> VoiceServerUpdate {
    return vc_update(value, "endpoint".to_owned());
}

fn check_connection_info(c_i: &ConnectionInfo) -> bool {
    return c_i.token.chars().count() > 0
        && c_i.endpoint.chars().count() > 0
        && c_i.session_id.chars().count() > 0;
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_target(false)
        .init();
   // tracing_subscriber::fmt().init();
    let stream_consumer: StreamConsumer = ClientConfig::new()
        .set("group.id", "voice")
        .set("bootstrap.servers", "localhost:9092")
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "false")
        .create()
        .expect("Consumer creation failed");

    stream_consumer
        .subscribe(&[&"discord-events"])
        .expect("Couldn't subscribe");

    let (tx, mut rx) = mpsc::channel::<VoiceServerUpdate>(10);

    tokio::spawn(async move {
        let mut connection_info = ConnectionInfo {
            guild_id: GuildId(NonZeroU64::new(810276987465760800).expect("Invalid guild")),
            user_id: UserId(NonZeroU64::new(1074051425207865384).expect("invalid user")),
            channel_id: Option::Some(ChannelId(
                NonZeroU64::new(810276988090581015).expect("invalid num"),
            )),
            token: String::from(""),
            endpoint: String::from(""),
            session_id: String::from(""),
        };

        while let Some(payload) = rx.recv().await {
            let s = payload.field.clone();
            if s.contains("token") {
                connection_info.token = payload.value;
            } else if s.contains("session_id") {
                connection_info.session_id = payload.value;
            } else if s.contains("endpoint") {
                connection_info.endpoint = payload.value;
            }

            if check_connection_info(&connection_info) {
                let cloned_connection_info = connection_info.clone();
                println!("{:?}", cloned_connection_info);
                tokio::spawn(async move {
                    let mut c = Call::standalone(
                        cloned_connection_info.guild_id,
                        cloned_connection_info.user_id,
                    );
                    c.connect(cloned_connection_info);
                    let music =
                        Input::from(File::new("/home/boomermath/discord_jukebox/music.mp3"));
                    c.play_input(music.into());
                });

                println!("Connected!");
            }
        }
    });

    tokio::spawn(async move {
        loop {
            let msg = stream_consumer.recv().await;
            let borrowed_msg = msg.expect("msg");
            if let Some(payload) = borrowed_msg.detach().payload() {
                let mut p = payload.to_owned();
                let json_dat = simd_json::to_borrowed_value(p.as_mut());
                let obj_val = json_dat.as_object().expect("not an object");

                if !obj_val["op"].as_i32().is_none() {
                    let op = obj_val["op"].as_i32().expect("no op!");
                    if op != 0 {
                        continue;
                    }
                }

                if !obj_val["t"].as_str().is_none() {
                    let t_val = obj_val["t"].as_str().expect("no t");
                    let d_val = obj_val["d"].as_object().expect("no data");
                    if t_val.contains("VOICE_SERVER_UPDATE") {
                        let token = String::from(d_val["token"].as_str().expect("no token"));
                        let endpoint = String::from(d_val["endpoint"].as_str().expect("no end"));
                        tx.send(data_token(token)).await.expect("token send");
                        tx.send(data_endpoint(endpoint))
                            .await
                            .expect("endpoint send");
                    } else if t_val.contains("VOICE_STATE") {
                        let session_d =
                            String::from(d_val["session_id"].as_str().expect("no s_id"));
                        tx.send(data_session(session_d)).await.expect("sesh send");
                    }
                }
            }
        }
    });

    match signal::ctrl_c().await {
        Ok(()) => {}
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {}", err);
            // we also shut down in case of error
        }
    }
}
