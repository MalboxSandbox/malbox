// Minimal illustrative YARA rule set for testing the malbox report pipeline.
// These rules are intentionally simple — they're here to trigger on common
// sample shapes during end-to-end testing, not to be a real detection engine.

rule PE_File
{
    meta:
        description = "Windows PE executable"
        mitre = "T1027"
        severity = "info"
    strings:
        $mz = { 4D 5A }
        $pe = { 50 45 00 00 }
    condition:
        $mz at 0 and $pe
}

rule ELF_File
{
    meta:
        description = "ELF executable (Linux/Unix)"
        severity = "info"
    strings:
        $elf = { 7F 45 4C 46 }
    condition:
        $elf at 0
}

rule PDF_File
{
    meta:
        description = "PDF document"
        severity = "info"
    strings:
        $header = "%PDF-"
    condition:
        $header at 0
}

rule ZIP_Archive
{
    meta:
        description = "ZIP or Office Open XML container"
        severity = "info"
    strings:
        $pk = { 50 4B 03 04 }
    condition:
        $pk at 0
}

rule Suspicious_Powershell_Encoded
{
    meta:
        description = "Encoded PowerShell command invocation"
        mitre = "T1059.001"
        severity = "suspicious"
    strings:
        $a = "powershell" nocase
        $b = "-enc" nocase
        $c = "-encodedcommand" nocase
        $d = "FromBase64String" nocase
    condition:
        $a and ($b or $c or $d)
}

rule Suspicious_Cmd_Download_Exec
{
    meta:
        description = "Command-line download + execute pattern"
        mitre = "T1105"
        severity = "suspicious"
    strings:
        $dl1 = "certutil" nocase
        $dl2 = "bitsadmin" nocase
        $dl3 = "curl " nocase
        $dl4 = "wget " nocase
        $exec1 = "cmd /c" nocase
        $exec2 = "cmd.exe /c" nocase
    condition:
        any of ($dl*) and any of ($exec*)
}

rule Suspicious_Process_Injection_Imports
{
    meta:
        description = "Imports typical of process injection"
        mitre = "T1055"
        severity = "malicious"
    strings:
        $a = "VirtualAllocEx"
        $b = "WriteProcessMemory"
        $c = "CreateRemoteThread"
        $d = "NtUnmapViewOfSection"
    condition:
        2 of them
}

rule Suspicious_Keylogger_Imports
{
    meta:
        description = "Imports typical of keylogger / credential theft"
        mitre = "T1056.001"
        severity = "malicious"
    strings:
        $a = "SetWindowsHookEx"
        $b = "GetAsyncKeyState"
        $c = "GetForegroundWindow"
        $d = "GetKeyboardState"
    condition:
        2 of them
}

rule EICAR_Test_String
{
    meta:
        description = "EICAR antivirus test string"
        severity = "malicious"
    strings:
        $eicar = "X5O!P%@AP[4\\PZX54(P^)7CC)7}$EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*"
    condition:
        $eicar
}
