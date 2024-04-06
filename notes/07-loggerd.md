# loggerd

This will be a logging daemon. It will take messages over a unix socket (maybe a pipe?) and write logs to a file. Because I want to support structured logging, indexing would be good so here's a basic format, based heavily on journald.

## Blocks

Every log file is a series of blocks. Blocks can be:

- Header block (the start of the file)
- Checkpoint block (hash of the data starting at the last checkpoint block)
- Entry block (list of pointers to field blocks)
- Field block (a k=v pair)
- Hash block (a key, and a list of buckets pointing to entry blocks with that k=v)

