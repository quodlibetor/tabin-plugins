(function() {var implementors = {};
implementors["http"] = [{"text":"impl&lt;'a, K, V, T&gt; TryFrom&lt;&amp;'a HashMap&lt;K, V, RandomState&gt;&gt; for HeaderMap&lt;T&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;K: Eq + Hash,<br>&nbsp;&nbsp;&nbsp;&nbsp;HeaderName: TryFrom&lt;&amp;'a K&gt;,<br>&nbsp;&nbsp;&nbsp;&nbsp;&lt;HeaderName as TryFrom&lt;&amp;'a K&gt;&gt;::Error: Into&lt;Error&gt;,<br>&nbsp;&nbsp;&nbsp;&nbsp;T: TryFrom&lt;&amp;'a V&gt;,<br>&nbsp;&nbsp;&nbsp;&nbsp;T::Error: Into&lt;Error&gt;,&nbsp;</span>","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a str&gt; for HeaderName","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a String&gt; for HeaderName","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a [u8]&gt; for HeaderName","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a str&gt; for HeaderValue","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a String&gt; for HeaderValue","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a [u8]&gt; for HeaderValue","synthetic":false,"types":[]},{"text":"impl TryFrom&lt;String&gt; for HeaderValue","synthetic":false,"types":[]},{"text":"impl TryFrom&lt;Vec&lt;u8&gt;&gt; for HeaderValue","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a [u8]&gt; for Method","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a str&gt; for Method","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a [u8]&gt; for StatusCode","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a str&gt; for StatusCode","synthetic":false,"types":[]},{"text":"impl TryFrom&lt;u16&gt; for StatusCode","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a [u8]&gt; for Authority","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a str&gt; for Authority","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a [u8]&gt; for PathAndQuery","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a str&gt; for PathAndQuery","synthetic":false,"types":[]},{"text":"impl TryFrom&lt;String&gt; for PathAndQuery","synthetic":false,"types":[]},{"text":"impl&lt;'_&gt; TryFrom&lt;&amp;'_ String&gt; for PathAndQuery","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a [u8]&gt; for Scheme","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a str&gt; for Scheme","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a [u8]&gt; for Uri","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a str&gt; for Uri","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a String&gt; for Uri","synthetic":false,"types":[]},{"text":"impl TryFrom&lt;String&gt; for Uri","synthetic":false,"types":[]},{"text":"impl TryFrom&lt;Parts&gt; for Uri","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a Uri&gt; for Uri","synthetic":false,"types":[]}];
implementors["nix"] = [{"text":"impl TryFrom&lt;i32&gt; for Signal","synthetic":false,"types":[]},{"text":"impl TryFrom&lt;u32&gt; for BaudRate","synthetic":false,"types":[]}];
implementors["reqwest"] = [{"text":"impl&lt;T&gt; TryFrom&lt;Request&lt;T&gt;&gt; for Request <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;T: Into&lt;Body&gt;,&nbsp;</span>","synthetic":false,"types":[]},{"text":"impl&lt;T&gt; TryFrom&lt;Request&lt;T&gt;&gt; for Request <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;T: Into&lt;Body&gt;,&nbsp;</span>","synthetic":false,"types":[]}];
implementors["tokio"] = [{"text":"impl TryFrom&lt;TcpListener&gt; for TcpListener","synthetic":false,"types":[]},{"text":"impl TryFrom&lt;TcpListener&gt; for TcpListener","synthetic":false,"types":[]},{"text":"impl TryFrom&lt;TcpStream&gt; for TcpStream","synthetic":false,"types":[]},{"text":"impl TryFrom&lt;TcpStream&gt; for TcpStream","synthetic":false,"types":[]}];
implementors["url"] = [{"text":"impl&lt;'a&gt; TryFrom&lt;&amp;'a str&gt; for Url","synthetic":false,"types":[]}];
if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()