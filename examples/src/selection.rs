// use rspc::{Router, RouterBuilder};
// use specta::{selection, Type};

// #[derive(Type)]
// pub struct User {
//     pub id: i32,
//     pub name: String,
//     pub age: i32,
//     pub password: String,
// }

// // We merge this router into the main router in `main.rs`.
// // This router shows how to do basic queries and mutations and how they tak
// pub fn mount() -> RouterBuilder {
//     Router::new()
//         .query("customSelection", |t| {
//             t(|_, _: ()| {
//                 // The user come from your database.
//                 let user = User {
//                     id: 1,
//                     name: "Monty Beaumont".to_string(),
//                     age: 7,
//                     password: "password".to_string(),
//                 };

//                 Ok::<(), rspc::Error>(selection!(user, { id, name, age }))
//             })
//         })
//         .query("customSelectionOnList", |t| {
//             t(|_, _: ()| {
//                 // The users come from your database.
//                 let users = vec![User {
//                     id: 1,
//                     name: "Monty Beaumont".to_string(),
//                     age: 7,
//                     password: "password".to_string(),
//                 }];
//                 Ok::<(), rspc::Error>(selection!(users, [{ id, name, age }]))
//             })
//         })
// }

fn main() {
    todo!("lmao, lol, rofl");
}
