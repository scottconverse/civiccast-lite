#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('Prepare','Build')][string]$Phase,
    [Parameter(Mandatory)][string]$WorkRoot,
    [Parameter(Mandatory)][string]$InnoCompiler,
    [Parameter(Mandatory)][string]$InnoUnpacker,
    [string]$Python = 'python',
    [string]$VendorInstaller,
    [string]$SignedEngine,
    [string]$SignedEngineReceipt
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $WorkRoot)) {
    New-Item -ItemType Directory -Path $WorkRoot -Force | Out-Null
}
$WorkRoot = (Resolve-Path -LiteralPath $WorkRoot).Path

$InnoCompiler = (Resolve-Path -LiteralPath $InnoCompiler).Path
$InnoUnpacker = (Resolve-Path -LiteralPath $InnoUnpacker).Path

function Get-Sha256 {
    [CmdletBinding()]
    param([Parameter(Mandatory)][string]$Path)
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

# --- Require InnoCompiler: exact SHA-256 + Authenticode Valid ---
$expectedIsccSha = '0a8757031b33777e4c9cbffee40f11a5062b36d25cbe144c1db73b6102b80ad7'
if (-not (Test-Path -LiteralPath $InnoCompiler -PathType Leaf)) {
    throw "InnoCompiler not found: $InnoCompiler"
}
$isccSha = Get-Sha256 -Path $InnoCompiler
if ($isccSha -ne $expectedIsccSha) {
    throw "InnoCompiler SHA-256 mismatch. Expected $expectedIsccSha; got $isccSha"
}
$isccSig = Get-AuthenticodeSignature -LiteralPath $InnoCompiler
if ($isccSig.Status -ne [System.Management.Automation.SignatureStatus]::Valid) {
    throw "InnoCompiler Authenticode status not Valid: $($isccSig.Status)"
}

# --- Require InnoUnpacker: exact SHA-256 ---
$expectedInnounpSha = '516be0ae6b7d7e402e31e6bc2c9f81f6f0b5552beb68e88676da7af4bd95b16d'
if (-not (Test-Path -LiteralPath $InnoUnpacker -PathType Leaf)) {
    throw "InnoUnpacker not found: $InnoUnpacker"
}
$innounpSha = Get-Sha256 -Path $InnoUnpacker
if ($innounpSha -ne $expectedInnounpSha) {
    throw "InnoUnpacker SHA-256 mismatch. Expected $expectedInnounpSha; got $innounpSha"
}

function Get-PinnedAssets {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$WorkRoot,
        [Parameter(Mandatory)][ValidateSet('Prepare','Build')][string]$Phase,
        [string]$VendorInstaller
    )

    $vendorUrl = 'https://gstreamer.freedesktop.org/data/pkg/windows/1.28.6/msvc/gstreamer-1.0-msvc-x86_64-1.28.6.exe'
    $vendorSha = '059251444d1267b486eba390b18d25fed87e10315e72f757ec6c7e912fa746b5'
    $ghBase    = 'https://raw.githubusercontent.com/GStreamer/cerbero/59548269f4fd0f701818f0bafdb102959ec81e65'

    $assets = @(
        [pscustomobject]@{ Name = 'gstreamer-1.0-msvc-x86_64-1.28.6.exe';     Url = $vendorUrl;                                                 Sha = $vendorSha }
        [pscustomobject]@{ Name = 'cerbero-base.iss';        Url = "$ghBase/data/inno/base.iss";                               Sha = '6b381b91c1bf90ac42faf639931231181c331320213d40a6ecdb40c1ee08636c' }
        [pscustomobject]@{ Name = 'background_inno.png';     Url = "$ghBase/packages/gstreamer-1.0/background_inno.png";       Sha = '67acea17305daf759b825d8cef6a77db805b803ce2852268422906d752ca8c83' }
        [pscustomobject]@{ Name = 'background_inno_dark.png';Url = "$ghBase/packages/gstreamer-1.0/background_inno_dark.png";  Sha = '6c0e24ccd93e27c85bb0644b4ab266faed4e6449c4d3825069adef50e7ee472a' }
        [pscustomobject]@{ Name = 'icon.png';                Url = "$ghBase/packages/gstreamer-1.0/icon.png";                   Sha = 'deeca9eb2773a12af048cae4a1ae563d5d6c081c66458c88bdbf357ac4173b28' }
        [pscustomobject]@{ Name = 'icon.ico';                Url = "$ghBase/packages/gstreamer-1.0/icon.ico";                   Sha = '887589553e699d2c9c3c64edf467360f01fbc5855a4c015397958a882e3d7a70' }
    )

    foreach ($a in $assets) {
        $dest = Join-Path -Path $WorkRoot -ChildPath $a.Name
        if (Test-Path -LiteralPath $dest -PathType Leaf) {
            $got = Get-Sha256 -Path $dest
            if ($got -ne $a.Sha) { throw "SHA-256 mismatch for $($a.Name). Expected $($a.Sha); got $got" }
            continue
        }
        if ($Phase -eq 'Build') { throw "$($a.Name) missing; Prepare must produce it." }
        if ($a.Name -eq 'gstreamer-1.0-msvc-x86_64-1.28.6.exe' -and -not [string]::IsNullOrWhiteSpace($VendorInstaller)) {
            if (-not (Test-Path -LiteralPath $VendorInstaller -PathType Leaf)) { throw "VendorInstaller not found: $VendorInstaller" }
            $got = Get-Sha256 -Path $VendorInstaller
            if ($got -ne $a.Sha) { throw "VendorInstaller SHA-256 mismatch. Expected $($a.Sha); got $got" }
            Write-Host "Using verified local vendor input: $VendorInstaller"
            Copy-Item -LiteralPath $VendorInstaller -Destination $dest -Force
        } else {
            Write-Host "Downloading $($a.Name) from $($a.Url)"
            & curl.exe --fail --silent --show-error --location --output $dest -- $a.Url
            if ($LASTEXITCODE -ne 0) { throw "curl failed ($LASTEXITCODE) downloading $($a.Url)" }
        }
        $got = Get-Sha256 -Path $dest
        if ($got -ne $a.Sha) { throw "SHA-256 mismatch for $($a.Name). Expected $($a.Sha); got $got" }
    }
}

function Expand-Inno {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$File,
        [Parameter(Mandatory)][string]$Tree
    )

    if (-not (Test-Path -LiteralPath $File -PathType Leaf)) {
        throw "Expand-Inno: File not found: $File"
    }
    if (Test-Path -LiteralPath $Tree) {
        throw "Expand-Inno: Tree already exists: $Tree"
    }

    $treeFull = [System.IO.Path]::GetFullPath($Tree)
    New-Item -ItemType Directory -Path $treeFull -Force | Out-Null

    $logPath = Join-Path -Path ([System.IO.Path]::GetTempPath()) -ChildPath ("innounp-" + [guid]::NewGuid().ToString('N') + ".log")

    $innounpArgs = @('-x','-m','-a','-b','-q',("-d" + $treeFull), $File)
    $output = & $script:InnoUnpacker @innounpArgs 2>&1
    $rc = $LASTEXITCODE
    $output | Out-File -FilePath $logPath -Encoding utf8 -Force
    if ($rc -ne 0) {
        throw "Expand-Inno: innounp exited with code $rc. Log: $logPath"
    }
    return $treeFull
}

function New-InstallerScript {
    param([Parameter(Mandatory=$true)][string]$Root)

    $src = Join-Path $Root 'vendor-original/install_script.iss'
    $dst = Join-Path $Root 'vendor-original/rebuild.iss'

    $text = [System.IO.File]::ReadAllText($src, [System.Text.UTF8Encoding]::new($false))

    # Remove final [Languages] section (stub)
    $idx = $text.LastIndexOf('[Languages]')
    if ($idx -ge 0) { $text = $text.Substring(0, $idx).TrimEnd() + "`r`n" }

    # Replace known Flags line
    $text = $text.Replace('Flags: logoutput waituntilidleterminated', 'Flags: logoutput')

    # Replace single-line settings (in-place)
    $text = [regex]::Replace($text, '(?im)^WizardStyle\s*=.*$', 'WizardStyle=modern dynamic windows11')
    $text = [regex]::Replace($text, '(?im)^Compression\s*=.*$', 'Compression=lzma2/max')
    $text = [regex]::Replace($text, '(?im)^PrivilegesRequiredOverridesAllowed\s*=.*$', 'PrivilegesRequiredOverridesAllowed=dialog commandline')
    $text = [regex]::Replace($text, '(?im)^WizardImageFile\s*=.*$', 'WizardImageFile="' + $Root + '/background_inno.png"')
    $text = [regex]::Replace($text, '(?im)^WizardSmallImageFile\s*=.*$', 'WizardSmallImageFile="' + $Root + '/icon.png"')
    $text = [regex]::Replace($text, '(?im)^ShowLanguageDialog\s*=.*\r?\n?', '')

    # Keys to append under [Setup]
    $appendKeys = @(
        'VersionInfoVersion=1.28.6'
        'WizardSizePercent=150'
        'DisableWelcomePage=no'
        'PrivilegesRequired=admin'
        'ChangesEnvironment=not is_portable_mode_enabled'
        'SetupLogging=yes'
        'UninstallLogging=yes'
        'CompressionThreads=auto'
        'LZMAUseSeparateProcess=yes'
        'SolidCompression=yes'
        'LZMANumBlockThreads=4'
        'SignedUninstaller=yes'
        'SignedUninstallerDir="' + $Root + '/uninstaller-probe"'
        'OutputDir="' + $Root + '/output"'
        'WizardImageFileDynamicDark="' + $Root + '/background_inno_dark.png"'
        'SetupIconFile="' + $Root + '/icon.ico"'
    )

    # Drop any pre-existing duplicates of those keys
    foreach ($k in $appendKeys) {
        $name = ($k -split '=', 2)[0]
        $text = [regex]::Replace($text, '(?im)^' + [regex]::Escape($name) + '\s*=.*\r?\n?', '')
    }

    # Insert keys at end of [Setup] section
    $m = [regex]::Match($text, '(?ms)^\[Setup\][^\r\n]*\r?\n(.*?)(?=^\[|\z)')
    if ($m.Success) {
        $pos = $m.Groups[1].Index + $m.Groups[1].Length
        $text = $text.Substring(0, $pos).TrimEnd() + "`r`n" + ($appendKeys -join "`r`n") + "`r`n`r`n" + $text.Substring($pos).TrimStart()
    }

    # Append #include
    $text = $text.TrimEnd() + "`r`n`r`n" + '#include "' + $Root + '/cerbero-base.iss"' + "`r`n"

    [System.IO.File]::WriteAllText($dst, $text, [System.Text.UTF8Encoding]::new($false))
}

function Get-PayloadManifest {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$Tree
    )
    $ErrorActionPreference = 'Stop'

    if (-not (Test-Path -LiteralPath $Tree -PathType Container)) {
        throw "Tree directory not found: $Tree"
    }
    $treeRoot = (Resolve-Path -LiteralPath $Tree).ProviderPath
    $issPath  = Join-Path $treeRoot 'install_script.iss'
    if (-not (Test-Path -LiteralPath $issPath -PathType Leaf)) {
        throw "install_script.iss not found under $treeRoot"
    }

    $lines = [System.IO.File]::ReadAllLines($issPath)
    $pattern = '^\s*Source:\s*"([^"]+)";'
    $manifest = [System.Collections.Generic.SortedDictionary[string,string]]::new(
        [System.StringComparer]::Ordinal)
    $treePrefix = $treeRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar,[System.IO.Path]::AltDirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar

    foreach ($line in $lines) {
        $m = [regex]::Match($line, $pattern)
        if (-not $m.Success) { continue }
        $rel = $m.Groups[1].Value -replace '\\','/'
        if ([string]::IsNullOrWhiteSpace($rel)) { throw "Empty source path" }
        if ($rel -match '(^|/)\.\.(/|$)') { throw "Unsafe path (traversal): $rel" }
        if ($rel -match '^[A-Za-z]:')     { throw "Unsafe path (drive): $rel" }
        if ([System.IO.Path]::IsPathRooted($rel)) { throw "Unsafe path (rooted): $rel" }
        $full = [System.IO.Path]::GetFullPath((Join-Path $treeRoot $rel))
        if (-not $full.StartsWith($treePrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Source path escapes tree: $rel"
        }
        if (-not [System.IO.File]::Exists($full)) { throw "Missing source file: $rel" }
        if ($manifest.ContainsKey($rel)) { throw "Duplicate source path: $rel" }
        $manifest[$rel] = (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()
    }

    if ($manifest.Count -ne 5450) {
        throw "Expected exactly 5450 Source entries; found $($manifest.Count)"
    }

    $ordered = [ordered]@{}
    foreach ($kv in $manifest.GetEnumerator()) { $ordered[$kv.Key] = $kv.Value }

    [pscustomobject]@{
        Tree        = $treeRoot
        IssPath     = $issPath
        Count       = $manifest.Count
        Entries     = $ordered
    }
}

function Get-FilesSection {
    [CmdletBinding()]
    param([Parameter(Mandatory)][string]$IssPath)
    $ErrorActionPreference = 'Stop'
    if (-not (Test-Path -LiteralPath $IssPath -PathType Leaf)) { throw "Missing $IssPath" }
    $lines = [System.IO.File]::ReadAllLines($IssPath)
    $inFiles = $false
    $entries = [System.Collections.Generic.List[string]]::new()
    foreach ($line in $lines) {
        $trim = $line.Trim()
        if ($trim -match '^\[(\w+)\]$') { $inFiles = ($Matches[1] -ieq 'Files'); continue }
        if (-not $inFiles) { continue }
        if ($trim.Length -eq 0)        { continue }
        if ($trim.StartsWith(';'))     { continue }
        $entries.Add($trim)
    }
    return $entries.ToArray()
}

function Compare-Payload {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$OriginalTree,
        [Parameter(Mandatory)][string]$RebuiltTree
    )
    $ErrorActionPreference = 'Stop'
    $origRoot = (Resolve-Path -LiteralPath $OriginalTree).ProviderPath
    $rebRoot  = (Resolve-Path -LiteralPath $RebuiltTree).ProviderPath

    $origIss = Join-Path $origRoot 'install_script.iss'
    $rebIss  = Join-Path $rebRoot  'install_script.iss'

    $origFiles = @(Get-FilesSection -IssPath $origIss)
    $rebFiles  = @(Get-FilesSection -IssPath $rebIss)

    if ($origFiles.Count -ne $rebFiles.Count) {
        throw "[Files] count mismatch: original=$($origFiles.Count) rebuilt=$($rebFiles.Count)"
    }
    $origText = $origFiles -join "`n"
    $rebText  = $rebFiles  -join "`n"
    if ($origText -cne $rebText) {
        for ($i = 0; $i -lt $origFiles.Count; $i++) {
            if ($origFiles[$i] -cne $rebFiles[$i]) {
                throw "[Files] mismatch @${i}`n original: $($origFiles[$i])`n rebuilt : $($rebFiles[$i])"
            }
        }
        throw "[Files] order/content mismatch"
    }

    $origManifest = Get-PayloadManifest -Tree $origRoot
    $rebManifest  = Get-PayloadManifest -Tree $rebRoot

    $ok = @($origManifest.Entries.Keys)
    $rk = @($rebManifest.Entries.Keys)
    if ($ok.Count -ne $rk.Count) { throw "Manifest key count mismatch" }
    for ($i = 0; $i -lt $ok.Count; $i++) {
        if ($ok[$i] -cne $rk[$i]) {
            throw "Manifest key mismatch @${i}: '$($ok[$i])' vs '$($rk[$i])'"
        }
        $h1 = $origManifest.Entries[$ok[$i]]
        $h2 = $rebManifest.Entries[$ok[$i]]
        if ($h1 -cne $h2) { throw "Hash mismatch for '$($ok[$i])': $h1 vs $h2" }
    }

    $origCCPath = Join-Path $origRoot 'embedded/CompiledCode.bin'
    $rebCCPath  = Join-Path $rebRoot  'embedded/CompiledCode.bin'
    if (-not [System.IO.File]::Exists($origCCPath)) { throw "CompiledCode.bin not found: $origCCPath" }
    if (-not [System.IO.File]::Exists($rebCCPath))  { throw "CompiledCode.bin not found: $rebCCPath" }
    $hCCo = (Get-FileHash -LiteralPath $origCCPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $hCCr = (Get-FileHash -LiteralPath $rebCCPath  -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($hCCo -cne $hCCr) {
        throw "CompiledCode.bin hash mismatch: $hCCo vs $hCCr"
    }

    [pscustomobject]@{
        FilesLines        = $origFiles.Count
        ManifestEntries   = $ok.Count
        CompiledCodeSha256 = $hCCo
        Match             = $true
    }
}

function Confirm-SignedEngine {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string]$Receipt
    )
    $ErrorActionPreference = 'Stop'

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf))    { throw "Target not found: $Path" }
    if (-not (Test-Path -LiteralPath $Receipt -PathType Leaf)) { throw "Receipt not found: $Receipt" }
    $p = (Resolve-Path -LiteralPath $Path).ProviderPath
    $r = (Resolve-Path -LiteralPath $Receipt).ProviderPath

    try {
        $json = Get-Content -LiteralPath $r -Raw -Encoding UTF8 | ConvertFrom-Json -ErrorAction Stop
    } catch {
        throw "Receipt is not valid JSON: $($_.Exception.Message)"
    }
    if ($null -eq $json) { throw "Receipt empty" }

    $required = @('unsigned_sha256','signed_sha256','signed_bytes','signer_subject','signer_thumb','timestamp')
    foreach ($f in $required) {
        $prop = $json.PSObject.Properties[$f]
        if (-not $prop -or $null -eq $prop.Value) { throw "Receipt missing/empty field: $f" }
    }

    $expectedUnsigned = '5f44c191d26825c23f3b63fe97f9c0a461b338f1d51e4546a501767f9fc7d19e'
    $unsigned = $json.unsigned_sha256.ToString().ToLowerInvariant()
    if ($unsigned -cne $expectedUnsigned) {
        throw "Receipt unsigned_sha256 mismatch: $unsigned"
    }

    $signedSha = $json.signed_sha256.ToString().ToLowerInvariant()
    $freshSha  = (Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($signedSha -cne $freshSha) {
        throw "Receipt signed_sha256 ($signedSha) != fresh file SHA-256 ($freshSha)"
    }

    $signedBytes = [long]$json.signed_bytes
    $actualBytes = (Get-Item -LiteralPath $p).Length
    if ($signedBytes -ne $actualBytes) {
        throw "signed_bytes mismatch: receipt=$signedBytes actual=$actualBytes"
    }

    $sig = Get-AuthenticodeSignature -LiteralPath $p
    if ($null -eq $sig) { throw "Unable to read Authenticode signature" }
    if ($sig.Status -ne [System.Management.Automation.SignatureStatus]::Valid) {
        throw "Authenticode status: $($sig.Status) - $($sig.StatusMessage)"
    }
    if ($null -eq $sig.SignerCertificate) { throw "No signer certificate present" }

    $signerSubject   = $sig.SignerCertificate.Subject
    $receiptSubject  = $json.signer_subject.ToString()
    if ($signerSubject -cne $receiptSubject) {
        throw "signer_subject mismatch: live='$signerSubject' receipt='$receiptSubject'"
    }

    $liveThumb    = $sig.SignerCertificate.Thumbprint.ToUpperInvariant()
    $receiptThumb = ($json.signer_thumb.ToString() -replace '\s','').ToUpperInvariant()
    if ($liveThumb -cne $receiptThumb) {
        throw "signer_thumb mismatch: live='$liveThumb' receipt='$receiptThumb'"
    }

    $cn = $null
    if ($signerSubject -match 'CN="([^"]+)"')      { $cn = $Matches[1].Trim() }
    elseif ($signerSubject -match 'CN=([^,]+)')    { $cn = $Matches[1].Trim() }
    if ($cn -ne 'Scott Converse') {
        throw "Signer CN must be 'Scott Converse'; got '$cn'"
    }

    if ($null -eq $sig.TimeStamperCertificate) { throw "Signature is not timestamped" }

    $liveTimestamp    = $sig.TimeStamperCertificate.Subject
    $receiptTimestamp = $json.timestamp.ToString()
    if ($liveTimestamp -cne $receiptTimestamp) {
        throw "timestamp mismatch: live='$liveTimestamp' receipt='$receiptTimestamp'"
    }

    [pscustomobject]@{
        Path             = $p
        UnsignedSha256   = $unsigned
        SignedSha256     = $signedSha
        SignedBytes      = $actualBytes
        SignerSubject    = $signerSubject
        SignerCN         = $cn
        SignerThumbprint = $liveThumb
        Timestamp        = $liveTimestamp
        AuthStatus       = $sig.Status.ToString()
        Valid            = $true
    }
}

$sha5f='5f44c191d26825c23f3b63fe97f9c0a461b338f1d51e4546a501767f9fc7d19e'
$unsigned=Join-Path $WorkRoot 'uninstaller-probe/uninst-6.7.3-5f44c191d2.e32'
$destEngine=Join-Path $WorkRoot 'gstreamer-1.28.6-embedded-setup-engine.exe'
$outer=Join-Path $WorkRoot 'output/gstreamer-1.0-msvc-x86_64-1.28.6.exe'
$iss=Join-Path $WorkRoot 'vendor-original/rebuild.iss'
$origTree=Join-Path $WorkRoot 'vendor-original'
$issLogPrepare=Join-Path $WorkRoot 'iscc-prepare.log'
$issLogBuild=Join-Path $WorkRoot 'iscc-build.log'

if ($Phase -eq 'Prepare') {
  if ((Get-ChildItem -LiteralPath $WorkRoot -Force | Measure-Object).Count -ne 0) { throw 'WorkRoot not empty' }
  Get-PinnedAssets -WorkRoot $WorkRoot -Phase $Phase -VendorInstaller $VendorInstaller | Out-Null
  Expand-Inno -File (Join-Path $WorkRoot 'gstreamer-1.0-msvc-x86_64-1.28.6.exe') -Tree $origTree | Out-Null
  New-Item -ItemType Directory -Force -Path (Join-Path $WorkRoot 'output'),(Join-Path $WorkRoot 'uninstaller-probe') | Out-Null
  New-InstallerScript -Root $WorkRoot
  & $InnoCompiler $iss *> $issLogPrepare
  if ($LASTEXITCODE -eq 0) { throw 'Prepare: expected nonzero ISCC exit' }
  if (-not (Select-String -LiteralPath $issLogPrepare -Pattern 'Signed uninstaller mode is enabled' -Quiet)) { throw 'Prepare: missing signed-mode log' }
  if ((Get-Sha256 -Path $unsigned) -ne $sha5f) { throw 'Unsigned engine hash mismatch' }
  Copy-Item -LiteralPath $unsigned -Destination $destEngine
  Write-Output $destEngine
  Write-Output (Get-Sha256 -Path $destEngine)
} elseif ($Phase -eq 'Build') {
  if ((Get-Sha256 -Path $destEngine) -ne $sha5f) { throw 'Unsigned digest mismatch' }
  Confirm-SignedEngine -Path $SignedEngine -Receipt $SignedEngineReceipt
  Get-PinnedAssets -WorkRoot $WorkRoot -Phase $Phase -VendorInstaller $VendorInstaller | Out-Null
  Copy-Item -LiteralPath $SignedEngine -Destination $unsigned -Force
  if ((Get-Sha256 -Path $unsigned) -ne (Get-Sha256 -Path $SignedEngine)) { throw 'E32 cache digest mismatch' }
  $rebuiltTree=Join-Path $WorkRoot 'rebuilt-payload'
  if (Test-Path -LiteralPath $outer) { throw 'Outer output already exists' }
  if (Test-Path -LiteralPath $rebuiltTree) { throw 'Verification tree already exists' }
  New-InstallerScript -Root $WorkRoot
  & $InnoCompiler $iss *> $issLogBuild
  if ($LASTEXITCODE -ne 0) { throw 'Build: ISCC failed' }
  if (-not (Select-String -LiteralPath $issLogBuild -Pattern 'Using existing signed uninstaller file' -Quiet)) { throw 'Build: missing existing-signed log' }
  $readback=Join-Path $WorkRoot 'engine-readback.e32'
  & $Python (Join-Path $PSScriptRoot 'read-inno-setup-engine.py') $outer $readback
  if (-not (Test-Path -LiteralPath $readback)) { throw 'Readback missing' }
  if ((Get-Sha256 -Path $readback) -ne (Get-Sha256 -Path $SignedEngine)) { throw 'Readback hash mismatch' }
  Expand-Inno -File $outer -Tree $rebuiltTree | Out-Null
  $cmp=Compare-Payload -OriginalTree $origTree -RebuiltTree $rebuiltTree
  if ($cmp.ManifestEntries -ne 5450) { throw "ManifestEntries $($cmp.ManifestEntries) != 5450" }
  [ordered]@{outer=$outer;outerSha256=(Get-Sha256 -Path $outer);manifestEntries=$cmp.ManifestEntries;signedEngineSha256=(Get-Sha256 -Path $SignedEngine)} |
    ConvertTo-Json | Set-Content -LiteralPath (Join-Path $WorkRoot 'output/build-report.json')
} else { throw "Unknown phase: $Phase" }
