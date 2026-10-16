//! Rust port of subset of [`luasocket`](https://github.com/lunarmodules/luasocket) API.

use std::{
    io::{self, BufRead, BufReader, Write},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    thread,
    time::{Duration, Instant},
};

use mlua::{Integer as LuaInteger, Lua, Result as LuaResult, Table, UserData, UserDataMethods};

pub fn create_module(lua: &Lua) -> LuaResult<Table> {
    let exports = lua.create_table()?;

    exports.set(
        "tcp4",
        lua.create_function(|_, ()| {
            Ok(LuaTcpIp4Server {
                listener: None,
                timeout: None,
            })
        })?,
    )?;

    Ok(exports)
}

/// Helper for constructing a `mlua::Error::BadArgument`
fn bad_argument(to: impl Into<String>, pos: usize, cause: mlua::Error) -> mlua::Error {
    #[allow(clippy::arc_with_non_send_sync)]
    mlua::Error::BadArgument {
        to: Some(to.into()),
        pos: pos + 1,
        name: None,
        cause: std::sync::Arc::new(cause),
    }
}

struct LuaTcpIp4Server {
    listener: Option<TcpListener>,
    timeout: Option<Duration>,
}

struct LuaTcpClient {
    reader: BufReader<TcpStream>,
    timeout: Option<Duration>,
}

impl UserData for LuaTcpIp4Server {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("bind", |_, this, (host, port): (String, u16)| {
            let addr = (host.as_str(), port)
                .to_socket_addrs()?
                .find(|a| a.is_ipv4());

            let Some(addr) = addr else {
                return Err(mlua::Error::external("no IPv4 address found"));
            };

            match TcpListener::bind(addr) {
                Ok(listener) => {
                    this.listener = Some(listener);
                    Ok(1)
                }
                Err(e) => Err(mlua::Error::external(e)),
            }
        });

        // `TcpListener::bind` handles the binding and immediately starts listening.
        // `listen` can therefore be a no-op if `bind` succeeded.
        methods.add_method("listen", |_, this, _backlog: u32| match &this.listener {
            Some(_) => Ok(1),
            None => Err(mlua::Error::external("socket is not bound")),
        });

        methods.add_method_mut("close", |_, this, ()| {
            this.listener = None;
            Ok(1)
        });

        methods.add_method("getsockname", |_, this, ()| match &this.listener {
            Some(listener) => {
                let addr = listener.local_addr()?;
                Ok((addr.ip().to_string(), addr.port()))
            }
            None => Err(mlua::Error::external("socket is not bound")),
        });

        methods.add_method_mut("settimeout", |_, this, seconds: f64| {
            this.timeout = if seconds < 0.0 {
                None
            } else {
                Some(Duration::from_secs_f64(seconds))
            };
            Ok(())
        });

        methods.add_method_mut("accept", |lua, this, ()| {
            let Some(listener) = &this.listener else {
                return Err(mlua::Error::external("socket is not bound"));
            };

            // the listener blocks indefinitely until a connection is established.
            // in order to set a timeout, we move it into nonblocking mode and repeatedly call
            // `accept`. if it returns `WouldBlock`, return if the deadline is reached or try again
            // after a short sleep.
            listener.set_nonblocking(true)?;
            let deadline = this.timeout.map(|t| Instant::now() + t);

            loop {
                match listener.accept() {
                    Ok((stream, _addr)) => {
                        let client = LuaTcpClient {
                            reader: BufReader::new(stream),
                            timeout: None,
                        };

                        let userdata = lua.create_userdata(client)?;
                        return Ok(userdata);
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        if deadline.is_some_and(|d| Instant::now() >= d) {
                            return Err(mlua::Error::external("timeout"));
                        }

                        thread::sleep(Duration::from_millis(20));
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(mlua::Error::external(e)),
                }
            }
        });
    }
}

impl UserData for LuaTcpClient {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("settimeout", |_, this, seconds: f64| {
            let timeout = if seconds < 0.0 {
                None
            } else {
                Some(Duration::from_secs_f64(seconds))
            };

            this.timeout = timeout;
            this.reader.get_ref().set_read_timeout(timeout)?;
            this.reader.get_ref().set_write_timeout(timeout)?;
            Ok(())
        });

        methods.add_method_mut("close", |_, this, ()| {
            let _ = this.reader.get_ref().shutdown(std::net::Shutdown::Both);
            Ok(())
        });

        // TODO: map correct errors to `closed` and `timeout`
        methods.add_method_mut(
            "receive",
            |_, this, (pattern, prefix, maxsize): (Option<String>, Option<String>, Option<u64>)| {
                if prefix.is_some() || maxsize.is_some() {
                    unimplemented!();
                }

                match pattern.as_deref().unwrap_or("*l") {
                    "*l" => {
                        let mut line = Vec::new();

                        match this.reader.read_until(b'\n', &mut line) {
                            Ok(0) => Err(mlua::Error::external("closed")),
                            Ok(_) => {
                                if line.last() == Some(&b'\n') {
                                    line.pop();
                                }
                                if line.last() == Some(&b'\r') {
                                    line.pop();
                                }

                                Ok(String::from_utf8_lossy(&line).into_owned())
                            }
                            Err(e)
                                if matches!(
                                    e.kind(),
                                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                                ) =>
                            {
                                Err(mlua::Error::external("timeout"))
                            }
                            Err(e) => Err(mlua::Error::external(e)),
                        }
                    }
                    "*a" => {
                        unimplemented!()
                    }
                    other => match other.parse::<u64>() {
                        Ok(_) => {
                            unimplemented!()
                        }
                        Err(_) => Err(bad_argument(
                            "receive",
                            1,
                            mlua::Error::external("invalid receive pattern"),
                        )),
                    },
                }
            },
        );

        // TODO: map correct errors to `closed` and `timeout`
        // TODO: handle i and j
        // TODO: on error, return index of the last byte (within [i, j]) sent
        methods.add_method_mut(
            "send",
            |_, this, (data, i, j): (String, Option<LuaInteger>, Option<LuaInteger>)| {
                if i.is_some() || j.is_some() {
                    unimplemented!()
                }

                match this.reader.get_mut().write_all(data.as_bytes()) {
                    Ok(()) => Ok(data.len()),
                    Err(e) => Err(mlua::Error::external(e)),
                }
            },
        );
    }
}
