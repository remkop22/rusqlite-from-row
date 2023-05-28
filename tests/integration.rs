use rusqlite::{params, Connection};
use rusqlite_from_row::FromRow;

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct Todo {
    id: i32,
    text: String,
    #[from_row(flatten, prefix = "author_")]
    author: User,
}

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct User {
    id: i32,
    name: String,
}

#[test]
fn from_row() {
    let connection = Connection::open_in_memory().unwrap();

    connection
        .execute_batch(
            "
            CREATE TABLE user (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL
            );

            CREATE TABLE todo (
                id INTEGER PRIMARY KEY, 
                text TEXT NOT NULL, 
                author_id INTEGER NOT NULL REFERENCES user(id)
            );
            ",
        )
        .unwrap();

    let user = connection
        .query_row(
            "INSERT INTO user(name) VALUES ('john') RETURNING id, name",
            [],
            User::try_from_row,
        )
        .unwrap();

    connection
        .execute(
            "INSERT INTO todo(text, author_id) VALUES ('laundy', ?1)",
            params![user.id],
        )
        .unwrap();

    let todo = connection
        .query_row(
            "SELECT t.id, t.text, u.id as author_id, u.name as author_name FROM todo t JOIN user u ON u.id = t.author_id WHERE t.author_id = ?1",
            params![user.id],
            Todo::try_from_row,
        )
        .unwrap();

    println!("{:#?}", todo);
}
