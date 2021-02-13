(function() {var implementors = {};
implementors["chrono"] = [{"text":"impl Sub&lt;FixedOffset&gt; for NaiveTime","synthetic":false,"types":[]},{"text":"impl Sub&lt;FixedOffset&gt; for NaiveDateTime","synthetic":false,"types":[]},{"text":"impl&lt;Tz:&nbsp;TimeZone&gt; Sub&lt;FixedOffset&gt; for DateTime&lt;Tz&gt;","synthetic":false,"types":[]},{"text":"impl Sub&lt;Duration&gt; for NaiveDate","synthetic":false,"types":[]},{"text":"impl Sub&lt;NaiveDate&gt; for NaiveDate","synthetic":false,"types":[]},{"text":"impl Sub&lt;Duration&gt; for NaiveDateTime","synthetic":false,"types":[]},{"text":"impl Sub&lt;NaiveDateTime&gt; for NaiveDateTime","synthetic":false,"types":[]},{"text":"impl Sub&lt;Duration&gt; for NaiveTime","synthetic":false,"types":[]},{"text":"impl Sub&lt;NaiveTime&gt; for NaiveTime","synthetic":false,"types":[]},{"text":"impl&lt;Tz:&nbsp;TimeZone&gt; Sub&lt;Duration&gt; for Date&lt;Tz&gt;","synthetic":false,"types":[]},{"text":"impl&lt;Tz:&nbsp;TimeZone&gt; Sub&lt;Date&lt;Tz&gt;&gt; for Date&lt;Tz&gt;","synthetic":false,"types":[]},{"text":"impl&lt;Tz:&nbsp;TimeZone&gt; Sub&lt;Duration&gt; for DateTime&lt;Tz&gt;","synthetic":false,"types":[]},{"text":"impl&lt;Tz:&nbsp;TimeZone&gt; Sub&lt;DateTime&lt;Tz&gt;&gt; for DateTime&lt;Tz&gt;","synthetic":false,"types":[]}];
implementors["hashbrown"] = [{"text":"impl&lt;T, S&gt; Sub&lt;&amp;'_ HashSet&lt;T, S&gt;&gt; for &amp;HashSet&lt;T, S&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;T: Eq + Hash + Clone,<br>&nbsp;&nbsp;&nbsp;&nbsp;S: BuildHasher + Default,&nbsp;</span>","synthetic":false,"types":[]}];
implementors["indexmap"] = [{"text":"impl&lt;T, S1, S2&gt; Sub&lt;&amp;'_ IndexSet&lt;T, S2&gt;&gt; for &amp;IndexSet&lt;T, S1&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;T: Eq + Hash + Clone,<br>&nbsp;&nbsp;&nbsp;&nbsp;S1: BuildHasher + Default,<br>&nbsp;&nbsp;&nbsp;&nbsp;S2: BuildHasher,&nbsp;</span>","synthetic":false,"types":[]}];
implementors["mio"] = [{"text":"impl Sub&lt;PollOpt&gt; for PollOpt","synthetic":false,"types":[]},{"text":"impl&lt;T:&nbsp;Into&lt;Ready&gt;&gt; Sub&lt;T&gt; for Ready","synthetic":false,"types":[]},{"text":"impl Sub&lt;UnixReady&gt; for UnixReady","synthetic":false,"types":[]}];
implementors["nix"] = [{"text":"impl Sub&lt;AtFlags&gt; for AtFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;OFlag&gt; for OFlag","synthetic":false,"types":[]},{"text":"impl Sub&lt;SealFlag&gt; for SealFlag","synthetic":false,"types":[]},{"text":"impl Sub&lt;FdFlag&gt; for FdFlag","synthetic":false,"types":[]},{"text":"impl Sub&lt;SpliceFFlags&gt; for SpliceFFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;FallocateFlags&gt; for FallocateFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;ModuleInitFlags&gt; for ModuleInitFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;DeleteModuleFlags&gt; for DeleteModuleFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;MsFlags&gt; for MsFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;MntFlags&gt; for MntFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;MQ_OFlag&gt; for MQ_OFlag","synthetic":false,"types":[]},{"text":"impl Sub&lt;FdFlag&gt; for FdFlag","synthetic":false,"types":[]},{"text":"impl Sub&lt;InterfaceFlags&gt; for InterfaceFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;PollFlags&gt; for PollFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;CloneFlags&gt; for CloneFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;EpollFlags&gt; for EpollFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;EpollCreateFlags&gt; for EpollCreateFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;EfdFlags&gt; for EfdFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;MemFdCreateFlag&gt; for MemFdCreateFlag","synthetic":false,"types":[]},{"text":"impl Sub&lt;ProtFlags&gt; for ProtFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;MapFlags&gt; for MapFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;MsFlags&gt; for MsFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;MlockAllFlags&gt; for MlockAllFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;Options&gt; for Options","synthetic":false,"types":[]},{"text":"impl Sub&lt;QuotaValidFlags&gt; for QuotaValidFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;SaFlags&gt; for SaFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;SfdFlags&gt; for SfdFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;SockFlag&gt; for SockFlag","synthetic":false,"types":[]},{"text":"impl Sub&lt;MsgFlags&gt; for MsgFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;SFlag&gt; for SFlag","synthetic":false,"types":[]},{"text":"impl Sub&lt;Mode&gt; for Mode","synthetic":false,"types":[]},{"text":"impl Sub&lt;FsFlags&gt; for FsFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;InputFlags&gt; for InputFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;OutputFlags&gt; for OutputFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;ControlFlags&gt; for ControlFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;LocalFlags&gt; for LocalFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;TimeSpec&gt; for TimeSpec","synthetic":false,"types":[]},{"text":"impl Sub&lt;TimeVal&gt; for TimeVal","synthetic":false,"types":[]},{"text":"impl Sub&lt;WaitPidFlag&gt; for WaitPidFlag","synthetic":false,"types":[]},{"text":"impl Sub&lt;AddWatchFlags&gt; for AddWatchFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;InitFlags&gt; for InitFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;AccessFlags&gt; for AccessFlags","synthetic":false,"types":[]}];
implementors["openssl"] = [{"text":"impl&lt;'a, 'b&gt; Sub&lt;&amp;'b BigNumRef&gt; for &amp;'a BigNumRef","synthetic":false,"types":[]},{"text":"impl&lt;'a, 'b&gt; Sub&lt;&amp;'b BigNum&gt; for &amp;'a BigNumRef","synthetic":false,"types":[]},{"text":"impl&lt;'a, 'b&gt; Sub&lt;&amp;'b BigNumRef&gt; for &amp;'a BigNum","synthetic":false,"types":[]},{"text":"impl&lt;'a, 'b&gt; Sub&lt;&amp;'b BigNum&gt; for &amp;'a BigNum","synthetic":false,"types":[]},{"text":"impl Sub&lt;CMSOptions&gt; for CMSOptions","synthetic":false,"types":[]},{"text":"impl Sub&lt;OcspFlag&gt; for OcspFlag","synthetic":false,"types":[]},{"text":"impl Sub&lt;Pkcs7Flags&gt; for Pkcs7Flags","synthetic":false,"types":[]},{"text":"impl Sub&lt;SslOptions&gt; for SslOptions","synthetic":false,"types":[]},{"text":"impl Sub&lt;SslMode&gt; for SslMode","synthetic":false,"types":[]},{"text":"impl Sub&lt;SslVerifyMode&gt; for SslVerifyMode","synthetic":false,"types":[]},{"text":"impl Sub&lt;SslSessionCacheMode&gt; for SslSessionCacheMode","synthetic":false,"types":[]},{"text":"impl Sub&lt;ShutdownState&gt; for ShutdownState","synthetic":false,"types":[]},{"text":"impl Sub&lt;X509CheckFlags&gt; for X509CheckFlags","synthetic":false,"types":[]},{"text":"impl Sub&lt;X509VerifyFlags&gt; for X509VerifyFlags","synthetic":false,"types":[]}];
implementors["tabin_plugins"] = [{"text":"impl Sub&lt;UserHz&gt; for UserHz","synthetic":false,"types":[]},{"text":"impl Sub&lt;Jiffies&gt; for Jiffies","synthetic":false,"types":[]},{"text":"impl Sub&lt;Calculations&gt; for Calculations","synthetic":false,"types":[]},{"text":"impl&lt;'a&gt; Sub&lt;&amp;'a Calculations&gt; for &amp;'a Calculations","synthetic":false,"types":[]}];
implementors["time"] = [{"text":"impl Sub&lt;Duration&gt; for Duration","synthetic":false,"types":[]},{"text":"impl Sub&lt;Duration&gt; for Timespec","synthetic":false,"types":[]},{"text":"impl Sub&lt;Timespec&gt; for Timespec","synthetic":false,"types":[]},{"text":"impl Sub&lt;SteadyTime&gt; for SteadyTime","synthetic":false,"types":[]},{"text":"impl Sub&lt;Duration&gt; for SteadyTime","synthetic":false,"types":[]},{"text":"impl Sub&lt;Duration&gt; for Tm","synthetic":false,"types":[]},{"text":"impl Sub&lt;Tm&gt; for Tm","synthetic":false,"types":[]}];
implementors["tokio"] = [{"text":"impl Sub&lt;Instant&gt; for Instant","synthetic":false,"types":[]},{"text":"impl Sub&lt;Duration&gt; for Instant","synthetic":false,"types":[]}];
if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()