// #[cfg(test)]
// mod tests {
//     use datafeed::auth::*;
//     use datafeed::misc_requests::*;
//     use std::env;
//     #[tokio::test]
//     async fn test_get_chart_token_with_user_data() {
//         let layout_id = "12345";
//         let user_data = Some(&datafeed::auth::UserData {
//             id: 12345,
//             session: "session_id".to_string(),
//             signature: "session_signature".to_string(),
//             username: todo!(),
//             session_hash: todo!(),
//             private_channel: todo!(),
//             auth_token: todo!(),
//         });
//         let token = get_chart_token(layout_id, user_data).await.unwrap();
//         // Assert that the token is not empty
//         assert!(!token.is_empty());
//     }
//     #[tokio::test]
//     async fn test_get_chart_token_without_user_data() {
//         let layout_id = "FiwrRse6";
//         let session = env::var("TV_SESSION").unwrap();
//         let signature = env::var("TV_SIGNATURE").unwrap();
//         let user = match get_user(&session, &signature, None).await {
//             Ok(user) => user,
//             Err(err) => panic!(
//                 "Cannot get user data with valid session, unwrap data error: {:#?}",
//                 err
//             ),
//         };
//         let user_data = Some(&user);
//         let result = get_chart_token(layout_id, user_data).await;
//         // Assert that an error is returned
//         assert!(result.is_err());
//     }
// }
