
cargo b
date now
print "let's go"
rm -f  target/debug/sock
plugin stop http

^sleep 6000 | h. serve { |req|
    seq 1 3 | each { sleep 1sec; $"hai: ($req.method)\n" }
}



