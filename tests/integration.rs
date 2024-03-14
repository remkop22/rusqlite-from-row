use std::{ffi::OsStr, marker::PhantomData, path::PathBuf};

use rusqlite::{params, Connection};
use rusqlite_from_row::FromRow;

#[derive(Debug, FromRow)]
pub struct Todo {
    id: i32,
    text: String,
    #[from_row(flatten, prefix = "author_")]
    author: User,
    #[from_row(flatten, prefix)]
    editor: User,
    #[from_row(flatten, default)]
    status: Status,
    #[from_row(default)]
    views: i32,
    #[from_row(from_fn = "<PathBuf as From<String>>::from")]
    file: PathBuf,
    #[from_row(skip)]
    empty: PhantomData<()>,
}

#[derive(Debug, FromRow, PartialEq, Eq, Default)]
pub struct Status {
    is_done: bool,
}

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct User {
    id: i32,
    name: String,
    #[from_row(flatten, prefix = "role_")]
    role: Option<Role>,
}

#[derive(Debug, FromRow)]
pub struct Role {
    id: i32,
    kind: String,
}

#[test]
fn from_row() {
    let connection = Connection::open_in_memory().unwrap();

    connection
        .execute_batch(
            "

            CREATE TABLE role (
                id INTEGER PRIMARY KEY,
                kind TEXT NOT NULL
            );

            CREATE TABLE user (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                role_id INTEGER NULL REFERENCES role(id)
            );

            CREATE TABLE status (
                id INTEGER PRIMARY KEY,
                is_done BOOL NOT NULL
            );

            CREATE TABLE todo (
                id INTEGER PRIMARY KEY, 
                text TEXT NOT NULL, 
                author_id INTEGER NOT NULL REFERENCES user(id),
                editor_id INTEGER NOT NULL REFERENCES user(id),
                views INTEGER NULL DEFAULT NULL,
                status_id INTEGER NULL REFERENCES status(id),
                file TEXT NOT NULL
            );
            ",
        )
        .unwrap();

    let role_id: i32 = connection
        .prepare("INSERT INTO role(kind) VALUES ('admin') RETURNING id")
        .unwrap()
        .query_row(params![], |r| r.get(0))
        .unwrap();

    let user_ids = connection
        .prepare("INSERT INTO user(name, role_id) VALUES ('john', ?1), ('jack', null) RETURNING id")
        .unwrap()
        .query_map([role_id], |r| r.get(0))
        .unwrap()
        .collect::<Result<Vec<i32>, _>>()
        .unwrap();

    let todo_id: i32 = connection
        .prepare(
            "INSERT INTO todo(text, author_id, editor_id, file) VALUES ('laundry', ?1, ?2, 'foo/bar.txt') RETURNING id",
        )
        .unwrap()
        .query_row(params![user_ids[0], user_ids[1]], |r| r.get(0))
        .unwrap();

    let todo = connection
        .query_row(
            "
            SELECT 
                t.id, 
                t.text, 
                t.views,
                t.file,
                a.id as author_id, 
                a.name as author_name,
                ar.id as author_role_id,
                ar.kind as author_role_kind,
                e.id as editor_id,
                e.name as editor_name,
                er.id as editor_role_id,
                er.kind as editor_role_kind,
                st.is_done as is_done
            FROM 
                todo t 
            JOIN user a ON 
                a.id = t.author_id 
            LEFT JOIN role ar ON 
                a.role_id = ar.id
            JOIN user e ON 
                e.id = t.editor_id
            LEFT JOIN role er ON 
                e.role_id = er.id
            LEFT JOIN status st ON
                t.status_id = st.id
            WHERE 
                t.id = ?1",
            params![todo_id],
            Todo::try_from_row,
        )
        .unwrap();

    assert_eq!(todo.id, todo_id);
    assert_eq!(todo.text, "laundry");
    assert_eq!(todo.status, Status { is_done: false });
    assert_eq!(todo.views, 0);
    assert_eq!(todo.file.file_name(), Some(OsStr::new("bar.txt")));
}
