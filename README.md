# rusqlite-from-row

Derive `FromRow` to generate a mapping between a struct and rusqlite rows.

```toml
[dependencies]
rusqlite_from_row = "0.2.2"
```

## Usage

Derive `FromRow` and execute a query that selects columns with the same names and types.

```rust
use rusqlite_from_row::FromRow;

#[derive(FromRow)]
struct Todo {
    todo_id: i32,
    text: String,
    author_id: i32,
}

let row = connection.query_row("SELECT todo_id, text, author_id FROM todos", [], Todo::try_from_row).unwrap();
```

### Nesting, Joins and Flattening

You might want to represent a join between two tables as nested structs. This is possible using the `#[from_row(flatten)]` on the nested field.
This will delegate the creation of that field to `FromRow::from_row` with the same row, instead of to `FromSql`. 

Because tables might have naming collisions when joining them, you can specify a `prefix = ".."` to retrieve the columns uniquely. This prefix should match the prefix you specify when renaming the column in a select, like `select <column> as <prefix><column>`. Nested prefixing is supported.

One can also use the `#[from_row(prefix)]` without a value. In this case the field name following a underscore will be used.

Outer joins can be supported by wrapping the flattened type in an `Option`. The `FromRow` implementation of `Option` will still require all columns to present, but will produce a `None` if all the columns are an SQL `null` value.

```rust
use rusqlite_from_row::FromRow;

#[derive(FromRow)]
struct Todo {
    id: i32,
    name: String,
    text: String,
    #[from_row(flatten, prefix = "user_")]
    author: User
    #[from_row(flatten, prefix)]
    editor: User
}

#[derive(FromRow)]
struct User {
    id: i32,
    name: String
}

// Rename all `User` fields to have `user_` or `editor_` prefixes.
let row = client
    .query_one(
        "
    SELECT 
        t.id, 
        t.name, 
        t.text, 
        u.name as user_name, 
        u.id as user_id,
        e.name as editor_name,
        e.id as editor_id
    FROM 
        todos t 
    JOIN 
        user u ON t.author_id = u.user_id
    JOIN
        user e ON t.editor_id = e.user_id
        ",
        [],
        Todo::try_from_row,
    )
    .unwrap();
```

### Renaming and Converting

If a struct contains a field with a name that differs from the name of the sql column, you can use the `#[from_row(rename = "..")]` attribute. 

Normally if you have a custom wrapper type like `struct DbId(i32)`, you'd need to implement `FromSql` in order to use it in a query. A simple alternative is to implement `From<i32>` or `TryFrom<i32>` for `DbId` and annotating a field with `#[from_row(from = "i32")]` or `#[from_row(try_from = "i32")]`.

This will delegate the sql conversion to `<i32 as FromSql>` and subsequently convert it to `DbId`.

```rust
struct DbId(i32);

impl From<i32> for DbId {
    fn from(value: i32) -> Self {
        Self(value)
    }
}

struct Todo {
    // If the sqlite column is named `todo_id`.
    #[from_row(rename = "todo_id", from = "i32")]
    id: i32,
    // If the sqlite column is `TEXT`, it will be decoded to `String`,
    // using `FromSql` and then converted to `Vec<u8>` using `std::convert::From`.
    #[from_row(from = "String")]
    todo: Vec<u8>
}
```


