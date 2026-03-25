import pyminusone

print("module:", pyminusone)
print("exports:", [n for n in dir(pyminusone) if not n.startswith("_") and callable(getattr(pyminusone, n))])

print("deobfuscate(js):", pyminusone.deobfuscate("js", "console.log(1+2)"))
print("deobfuscate_with(js):", pyminusone.deobfuscate_with("javascript", "console.log(1+2)", ["ParseInt", "SubAddInt"]))
print("deobfuscate_without(js):", pyminusone.deobfuscate_without("js", "console.log(1+2)", ["MultInt"]))

print("deobfuscate(ps1):", pyminusone.deobfuscate("ps", "Write-Host (1+2)"))
print("deobfuscate_with(ps1):", pyminusone.deobfuscate_with("powershell", "Write-Host (1+2)", ["ParseInt", "AddInt", "Forward"]))
print("deobfuscate_without(ps1):", pyminusone.deobfuscate_without("ps1", "Write-Host (1+2)", ["MultInt"]))