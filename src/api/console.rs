use mlua::{Function, Lua, MultiValue, Result as LuaResult, Table, Value};
use std::io::{Write, stdout};

pub fn console_printf(l: &Lua, (fmt, args): (String, MultiValue)) -> LuaResult<()> {
    // uses lua's builtin string.format function
    let string_module: Table = l.globals().get("string")?;
    let format_func: Function = string_module.get("format")?;
    let formatted_string = format_func.call::<String>((fmt, args))?;
    println!("{formatted_string}");
    Ok(())
}

pub fn console_execute(_l: &Lua, _cmd: String) -> LuaResult<()> {
    Ok(())
}

pub fn console_clear(_l: &Lua, _: ()) -> LuaResult<()> {
    Ok(())
}

pub fn console_print_table(
    _l: &Lua,
    (table, no_recursive): (Table, Option<bool>),
) -> LuaResult<()> {
    print_table(&table, 0, !no_recursive.unwrap_or(true))?;
    Ok(())
}

fn print_table(table: &Table, indent: usize, recursive: bool) -> LuaResult<()> {
    let mut lock = stdout().lock();
    writeln!(lock, "{{")?;
    for pair in table.pairs::<Value, Value>() {
        let inner_ind = indent + 2;
        let (key, value) = pair?;

        if key.is_string() {
            write!(lock, "{0:>1$}\"{2}\" = ", "", inner_ind, key.to_string()?,)?;
        } else {
            write!(lock, "{0:>1$}{2} = ", "", inner_ind, key.to_string()?,)?;
        }

        if value.is_table() {
            if recursive {
                print_table(value.as_table().unwrap(), indent + 2, recursive)?;
            } else {
                writeln!(lock, "{}", value.to_string()?)?;
            }
        } else if value.is_string() {
            writeln!(lock, "\"{}\"", value.to_string()?)?;
        } else {
            writeln!(lock, "{}", value.to_string()?)?;
        }
    }
    writeln!(lock, "{0:>1$}}}", "", indent)?;
    Ok(())
}
