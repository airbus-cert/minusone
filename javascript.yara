rule js_eval: alert {
  strings:
    $eval_plain      = " eval("
    $eval_encoded    = /\beval\s*\(\s*["']/
    $settimeout_str  = /\bsetTimeout\s*\(\s*["']/
    $setinterval_str = /\bsetInterval\s*\(\s*["']/

  condition:
    any of them
}

rule js_function_constructor: alert {
  strings:
    $func_call = /new\s+Function\s*\(/

  condition:
    any of them
}

rule js_constructor_chain: alert {
  strings:
    $ctor_chain      = /\.constructor\s*\(/
    $ctor_bracket_dq = "[\"constructor\"]"
    $ctor_bracket_sq = "['constructor']"

  condition:
    any of them
}

rule js_global_fn_call_from_str: alert {
  strings:
    $gfcfs      = /this\[.*\]\(/ //example: this['ev' + 'al']...

  condition:
    any of them
}

// to do the same trick as the constructor chain
rule js_prototype_pollution: alert {
  strings:
    $proto_rw  = ".prototype"
    $proto_assign  = "__proto__["
    $prototype_set = /Object\.prototype\.[A-Za-z_$'"]\w*\s*=/
    $prototype_bracket = "['prototype']"

  condition:
    any of them
}

rule js_shell_script_exec: alert {
  strings:
    $wscript_shell              = "WScript.Shell" nocase
    $wscript_createobject       = "WScript.CreateObject" nocase
    $shell_application          = "Shell.Application" nocase
    $scripting_filesystemobject = "Scripting.FileSystemObject" nocase

  condition:
    any of them
}

rule js_windows_http_req: alert {
  strings:
    $xmlhttp = "MSXML2.XMLHTTP" nocase
    $adodb   = "ADODB.Stream" nocase
    $winhttp = "WinHttp.WinHttpRequest" nocase

  condition:
    any of them
}

rule js_encoding: alert {
  strings:
    $charcode         = /String\.fromCharCode\s*\(/
    $atob             = "atob("
    $buffer_from_b64  = /Buffer\.from\(.+,\s*['"]base64['"]\s*\)/
    $buffer_alloc_b64 = /Buffer\.alloc\(.+,.+,\s*['"]base64['"]\s*\)/
    $unescape         = /unescape\s*\(/
    $decode_uri       = /decodeURIComponent\s*\(/
    $b64_blob         = /[A-Za-z0-9+\/]{60,}=/

  condition:
    any of them
}

rule js_massive_string: alert {
  strings:
    $massive = /("(?:[^"\\\r\n]|\\.){256,}")|('(?:[^'\\\r\n]|\\.){256,}')/

  condition:
    any of them
}

rule js_node_child_process: alert {
  strings:
    $require_cp = /require\s*\(\s*['"]child_process['"]\s*\)/
    $exec       = /\bexecSync\s*\(/
    $spawn      = /\bspawnSync?\s*\(/
    $execfile   = /\bexecFileSync?\s*\(/

  condition:
    any of them
}

rule js_node_fs: alert {
  strings:
    $require_fs = /require\s*\(\s*['"]fs['"]\s*\)/
    $write_file = /\bwriteFileSync?\s*\(/
    $read_file  = /\breadFileSync?\s*\(/
    $unlink     = /\bunlinkSync?\s*\(/

  condition:
    any of them
}

rule js_node_net_raw: alert {
  strings:
    $require_net = /require\s*\(\s*['"]net['"]\s*\)/
    $createconn  = /\bnet\.createConnection\s*\(/

  condition:
    any of them
}

rule js_windows_shell_cmd: alert {
  strings:
    $powershell  = "powershell" nocase
    $cmd_exe     = "cmd.exe" nocase
    $mshta       = "mshta" nocase
    $rundll32    = "rundll32" nocase
    $regsvr32    = "regsvr32" nocase
    $wscript_exe = "wscript.exe" nocase
    $cscript_exe = "cscript.exe" nocase

  condition:
    any of them
}

rule js_reverse_shell_indicators: alert {
  strings:
    $socket_pipe  = /\.pipe\s*\(\s*sock/
    $stdin_socket = /process\.stdin\.pipe/
    $proc_stdout  = /process\.stdout\.pipe/

  condition:
    2 of them
}

rule js_char_escape: alert {
  strings:
    // \xXX
    // $hex_escape     = /((\\x[0-9a-fA-F]{2})\s*){4,}/
    // \uXXXX or \u{XX...X}  the second one can have between 1 and inf hex chars
    // $unicode_escape = /(((\\u[0-9a-fA-F]{4})|(\\u\{[0-9a-fA-F]+\}))\s*){3,}/
    $mixed_escape = /((\\u[0-9a-fA-F]{4})|(\\u\{[0-9a-fA-F]+\})|((\\x[0-9a-fA-F]{2})\s*)){4,}/

  condition:
    any of them
}

rule js_charat_in_a_for: alert {
  strings:
    $charat_for = /for\s*\(.*\)\s*\{[^}]*\.charAt\s*\(/

  condition:
    any of them
}

rule js_dynamic_funcname_via_bracket: alert {
  strings:
    // .substr(0, <variable>)  extracting a substring of computed length
    $substr_var  = /\.substr\s*\(\s*0\s*,\s*[A-Za-z_$][A-Za-z0-9_$]*\s*\)/
    // var x = SomeObj[decoded_var]  bracket property lookup on a non-array
    $bracket_dyn = /var\s+\w+\s*=\s*[A-Za-z_$][A-Za-z0-9_$]*\s*\[\s*[A-Za-z_$][A-Za-z0-9_$]+\s*\]/
    // p.join('')  reassembling shuffled char array
    $join_empty  = /\.join\s*\(\s*['"]{2}\s*\)/

  condition:
    $substr_var and $bracket_dyn and $join_empty
}

rule js_javascript_obfuscator_com_or_suspicious_str_operations: alert {
  strings:
    $charat_populate = /\[\s*\w+\s*\]\s*=\s*\w+\.charAt\s*\(\s*\w+\s*\)/  // p[w] = e.charAt(w)
    $swap_pair       = /\[\s*\w+\s*\]\s*=\s*\w+\s*\[\s*\w+\s*\]\s*;[^;]{0,40}\[\s*\w+\s*\]\s*=\s*\w+\s*;/  // p[t]=p[i]; p[i]=k
    $join_rebuild    = /\.join\s*\(\s*['"]{2}\s*\)/  // .join('')
    $substr_zero     = /\.substr\s*\(\s*0\s*,/  // .substr(0,
    $bracket_lookup  = /=\s*[A-Za-z_$]\w*\s*\[\s*[A-Za-z_$]\w+\s*\]/  // = obj[varname]

  condition:
    $charat_populate and $swap_pair and $join_rebuild and ($substr_zero or $bracket_lookup)
}

// https://obfuscator.io/legacy-playground (the most used)
rule js_obfuscator_io: alert {
  strings:
    $obfuscated_name       = "_0x"
    $self_defending        = "(((.+)+)+)+$"
    $disable_console_error = /\,\s*\'error\'\s*\,/
    $disable_console_warn  = /\,\s*\'warn\'\s*\,/
    $disable_console_trace = /\,\s*\'trace\'\s*\,/
    $disable_console_info  = /\,\s*\'info\'\s*\,/

  condition:
    #obfuscated_name > 10 or
    #self_defending > 0 or
    // disable console
    (#disable_console_error > 0 and #disable_console_warn > 0 and #disable_console_trace > 0 and #disable_console_info > 0)
}

// https://obfuscate.js.org
rule js_obfuscate_js_org: alert {
  strings:
    $dec_payload_colon_splitted = /(\d+\:){100}/

  condition:
    any of them
}

// https://www.freejsobfuscator.com (be aware of what you obfuscate, this is closed source and everything is sent to a server)
rule js_free_js_obfuscator_com: alert {
  strings:
    $illogic_operations = "+!-~~[]"

  condition:
    any of them
}

rule js_jsfuck : alert {
  strings:
    $jsfuck = /(?:[\(\)\[\]\+!]\s*){20,}/

  condition:
    any of them
}

rule js_windows_persistence: alert {
  strings:
    $run_key     = /Software\\\\?Microsoft\\\\?Windows\\\\?CurrentVersion\\\\?Run/ nocase
    $hkcu        = "HKCU\\" nocase
    $hklm        = "HKLM\\" nocase
    $schtasks    = "schtasks" nocase
    $reg_add     = /\breg(\.exe)?\s+add\b/ nocase
    $startup_dir = "\\Start Menu\\Programs\\Startup" nocase
    $wmi_event   = "__EventFilter" nocase  // WMI persistence

  condition:
    any of them
}

rule js_defender_amsi_evasion: alert {
  strings:
    $amsi_buffer    = "AmsiScanBuffer" nocase
    $amsi_utils     = "AmsiUtils" nocase
    $amsi_dll       = "amsi.dll" nocase
    $mp_pref        = "Set-MpPreference" nocase
    $disable_rtmon  = "DisableRealtimeMonitoring" nocase
    $add_preference = "Add-MpPreference" nocase
    $set_preference = "Set-MpPreference" nocase

  condition:
    any of them
}

// useless ?
rule js_common_exfil_endpoints: alert {
  strings:
    $discord_wh   = /discord(app)?\.com\/api/ nocase
    $telegram_api = "api.telegram.org" nocase
    $pastebin_raw = "pastebin.com/raw" nocase
    $gh_raw       = "raw.githubusercontent.com"
    $ipify        = "api.ipify.org"
    $ngrok        = ".ngrok.io" nocase

  condition:
    any of them
}

rule js_stealer_paths: alert {
  strings:
    $chrome_userdata = /Google\\\\?Chrome\\\\?User Data/ nocase
    $login_data      = "Login Data" nocase
    $local_state     = "Local State" nocase
    $cookies_db      = "\\\\Cookies" nocase
    $ff_profiles     = /Mozilla\\\\?Firefox\\\\?Profiles/ nocase
    $edge_userdata   = /Edge\\\\?User Data/ nocase

  condition:
    any of them
}

rule js_suspicious_operations: alert {
  strings:
    $nonsense      = /((\[\]|\'\'|\"\")\s*[\-*\/~])|([+\-*\/~!]\s*(\[\]|\'\'|\"\"))/
    $xor           = "^="
    $boolnotnot    = "!!"
    $bitwisenotnot = "~~"

  condition:
    #nonsense + #xor + #boolnotnot + #bitwisenotnot >= 20
}

rule js_many_string_concat: alert {
  strings:
    $concat = /['"]\s*\+\s*['"]/
  condition:
    #concat >= 30
}


rule js_many_typeof: alert {
  strings:
    $typeof = "typeof"

  condition:
    #typeof >= 5
}

rule js_many_non_decimal_number: alert {
  strings:
    $bin = /0b[01]+/ nocase
    $oct = /0o[0-7]+/ nocase
    $hex = /0x[0-9a-f]+/ nocase

  condition:
    #bin + #oct + #hex >= 25
}

// https://jamtg.github.io/aaencode-and-aadecode/
rule js_aaencode: alert {
  strings:
    // jj version
    $aaencode0 = "ﾟωﾟ"
    $aaencode1 = "┻━┻"
    $aaencode2 = "o^_^o"
    $aaencode3 = "ﾟΘﾟ"
    $aaencode4 = "ﾟДﾟ"
    // $ version
    $aaencode5 = "$$$"
    $aaencode6 = "(![]+\"\")[$]"
    $aaencode7 = "(!\"\"+\"\")[$]"
    $aaencode8 = "++$"
    $aaencode9 = "$$__"

  condition:
    #aaencode0 + #aaencode1 + #aaencode2 + #aaencode3 + #aaencode4 + #aaencode5 + #aaencode6 + #aaencode7 + #aaencode8 + #aaencode9 >= 20
}
