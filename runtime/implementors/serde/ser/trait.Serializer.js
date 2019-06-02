(function() {var implementors = {};
implementors["bincode"] = [{text:"impl&lt;'a, W:&nbsp;<a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/std/io/trait.Write.html\" title=\"trait std::io::Write\">Write</a>, E:&nbsp;<a class=\"trait\" href=\"byteorder/trait.ByteOrder.html\" title=\"trait byteorder::ByteOrder\">ByteOrder</a>&gt; <a class=\"trait\" href=\"serde/ser/trait.Serializer.html\" title=\"trait serde::ser::Serializer\">Serializer</a> for &amp;'a mut <a class=\"struct\" href=\"bincode/internal/struct.Serializer.html\" title=\"struct bincode::internal::Serializer\">Serializer</a>&lt;W, E&gt;",synthetic:false,types:["bincode::ser::Serializer"]},];
implementors["toml"] = [{text:"impl&lt;'a, 'b&gt; <a class=\"trait\" href=\"serde/ser/trait.Serializer.html\" title=\"trait serde::ser::Serializer\">Serializer</a> for &amp;'b mut <a class=\"struct\" href=\"toml/ser/struct.Serializer.html\" title=\"struct toml::ser::Serializer\">Serializer</a>&lt;'a&gt;",synthetic:false,types:["toml::ser::Serializer"]},];

            if (window.register_implementors) {
                window.register_implementors(implementors);
            } else {
                window.pending_implementors = implementors;
            }
        
})()
