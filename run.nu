
cargo b
date now
print "let's go"
rm -f  target/debug/sock
plugin stop http

# ^sleep 6000 | h. serve { |req| $in }

# ^sleep 6000 | h. serve { "hello world." }

# let c = { |req| let foo = $in; echo "hi"; $foo }
# let c = { |req| prepend {meta: 123 } }

# let c = { |req| let var = "foo"; if $req.method == "GET" { each { str upcase } } else { "404" } }

def path-to-store [x] {
    "./store" | path join ($x | str trim -l -c "/")
}

let c = { |req|
    match $req {
        { method: "GET" } => (open (path-to-store $req.path)),
        { method: "POST" } => (save -f (path-to-store $req.path); "ok"),
        _ => "404"
    }
}

# let c = { print "hi"}

^sleep 6000 | h. serve $c
