
cargo b
date now
print "let's go"
rm -f  target/debug/sock
plugin stop http

# ^sleep 6000 | h. serve { |req| $in }

let c = { |req| $in }
^sleep 6000 | h. serve $c


