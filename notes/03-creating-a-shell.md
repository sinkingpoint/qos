# Creating a shell

A shell sounds like something useful to have. Something that I can type commands into and run binaries. Let's explore what's required here.

Off the bat, I'm gonna need a parser - https://pest.rs/

Maybe stty settings? - that'll need ioctls

https://ftp.gnu.org/old-gnu/Manuals/glibc-2.2.3/html_chapter/libc_17.html
https://ftp.gnu.org/old-gnu/Manuals/glibc-2.2.3/html_chapter/libc_toc.html#TOC578

> When a shell program that normally performs job control is started, it has to be careful in case it has been invoked from another shell that is already doing its own job control. 

We'll need a process group

getpgid/setpgid etc

https://docs.rs/nix/latest/nix/ / https://github.com/nix-rust/nix


https://www.gnu.org/software/libc/manual/html_node/Job-Control-Signals.html - SIGTTIN

---

```
{"msg":"failed to run tcgetpgrp. Err: -ENOTTY: Not a typewriter","level":"ERRO","ts":"2023-12-29T07:22:16.983349661Z"}
```

Hrmm https://pubs.opengroup.org/onlinepubs/009696699/functions/tcgetpgrp.html

> The calling process does not have a controlling terminal, or the file is not the controlling terminal.

What's a controlling terminal?

Seems like `sh` does this as well, so probably innocuous.

```
init: cannot set terminal process group (-1): Inappropriate ioctl for device
init: no job control in this shell
```

https://gist.github.com/notheotherben/819ad3a3ada4a05e6fcbd9fcb27a992f

---

Parsing:

PEG Parser? https://pest.rs

Something like:

```
alpha        =  { 'a'..'z' | 'A'..'Z' }
digit        =  { '0'..'9' }
double_quote =  { "\"" }
escape       =  { "\\" }
COMMENT      = @{ "#" ~ (!"\n" ~ ANY)* }
WHITESPACE   = _{ " " }

expression = { SOI ~ env_variable* ~ string* EOI }

env_variable     =  { env_variable_key ~ "=" ~ string }
env_variable_key = @{ alpha ~ (alpha | digit | "_")* }

string = { unquoted_string | single_quoted_string | double_quoted_string }

unquoted_string = @{ (unquoted_char | escaped_char)+ }
unquoted_char   = @{ !(('\u{00}'..'\u{20}') | "\"" | "'" | "\\") ~ ANY }

single_quoted_string         = @{ "'" ~ (single_quoted_escaped_char | single_quoted_unescaped_char)* ~ "'" }
single_quoted_escaped_char   = @{ escaped_char | (escape ~ "'") }
single_quoted_unescaped_char = @{ !('\u{00}'..'\u{1f}' | "\'" | "\\") ~ ANY }

double_quoted_string         = @{ "\"" ~ (double_quoted_escaped_char | double_quoted_unescaped_char)* ~ "\"" }
double_quoted_escaped_char   = @{ escaped_char | (escape ~ "\"") }
double_quoted_unescaped_char = @{ !('\u{00}'..'\u{1f}' | "\"" | "\\") ~ ANY }

escaped_char = @{ escape ~ ("\\" | "b" | "f" | "n" | "r" | "t" | ("u" ~ ASCII_HEX_DIGIT{2, 4})) }
```

Let's build our own.

