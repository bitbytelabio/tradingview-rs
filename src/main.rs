extern crate google_authenticator;

use std::borrow::Borrow;

use tracing::{debug, info};

mod auth;
mod misc_requests;
mod utils;

pub static UA: &'static str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36 uacq";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    // info!("Starting up");
    // let indicators = misc_requests::get_builtin_indicators().await?;
    // debug!("Indicators: {:?}", indicators);

    // println!("{}", utils::client::gen_session_id("qs"));
    // let user = auth::get_user(
    //     "wow63q1l614ilkutj8tkc7zwhp87e09b",
    //     "v1:I6szYEiR40S888deJb6fJ33fnOVLGw1JbhP+7Hw63+U=",
    //     None,
    // )
    // .await
    // .unwrap();

    // let user = auth::login_user("batttheyshool0211", "batttheyshool0211").await?;
    // info!("User: {:#?}", user);

    // let data = misc_requests::get_private_indicators(
    //     "wow63q1l614ilkutj8tkc7zwhp87e09b",
    //     "v1:I6szYEiR40S888deJb6fJ33fnOVLGw1JbhP+7Hw63+U=",
    // )
    // .await?;
    // info!("Data: {:#?}", data);
    // let totp = TOTP::from_url(
    //     "otpauth://totp/TradingView:lite_bitbytelab?secret=PTB2JVFN3YXVGVFX&issuer=TradingView",
    // )
    // .unwrap();
    // let code = totp.generate_current().unwrap();
    // println!("{}", code);
    // let totp = TOTP::new(
    //     Algorithm::SHA512,
    //     6,
    //     1,
    //     30,
    //     Secret::Encoded("KBKEEMSKKZDE4M2ZLBLEOVSGLAFA".to_string())
    //         .to_bytes()
    //         .unwrap(),
    //     Some("TradingView".to_string()),
    //     "lite_bitbytelab".to_string(),
    // )
    // .unwrap();
    // let token = totp.generate_current().unwrap();
    // println!("{}", token);

    // let mut rfc = Rfc6238::with_defaults(
    //     Secret::Encoded("KBKEEMSKKZDE4M2ZLBLEOVSGLAFA".to_string())
    //         .to_bytes()
    //         .unwrap(),
    // )
    // .unwrap();

    // // optional, set digits
    // // rfc.digits(8).unwrap();

    // // create a TOTP from rfc
    // let totp = TOTP::from_rfc6238(rfc).unwrap();
    // let code = totp.generate_current().unwrap();
    // println!("code: {}", code);

    // println!("{}", get_code!("PTB2JVFN3YXVGVFX").unwrap());
    // let user_data = auth::login_user(
    //     "lite_bitbytelab",
    //     "dAIuLpdzmEy8HWnIYRGwigRA4XwJT4Ny/WIsD/rXy5qurJwu",
    //     Some("PTB2JVFN3YXVGVFX".to_string()),
    // )
    let user_data = auth::get_user(
        "wow63q1l614ilkutj8tkc7zwhp87e09b",
        "v1:I6szYEiR40S888deJb6fJ33fnOVLGw1JbhP+7Hw63+U=",
        None,
    )
    .await?;
    let token = misc_requests::get_chart_token("FiwrRse6", Some(&user_data))
        .await
        .unwrap();
    info!("Token: {}", token);
    Ok(())
}
