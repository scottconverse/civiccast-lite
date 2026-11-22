$script:CivicCastSignedGStreamerUrl = 'https://github.com/scottconverse/civiccast-lite/releases/download/v0.1.0.13/gstreamer-1.28.6-windows-x64-civiccast-signed.exe'
$script:CivicCastSignedGStreamerFile = 'gstreamer-1.28.6-windows-x64-civiccast-signed.exe'
$script:CivicCastSignedGStreamerReceiptFile = 'halo-gstreamer-1.28.6-rebuilt-setup.exe'
$script:CivicCastSignedGStreamerSignedSha256 = 'daf7cf53c5ed1d58b802143d34fea6645d8ac92c3a954e0219d7d3a42b135d4f'
$script:CivicCastSignedGStreamerUnsignedSha256 = '678cac58e182b5aa3e71fe16451e5b29b5eec54d11cc919b843e0ac4b73670d0'
$script:CivicCastSignedGStreamerSignedBytes = [long]528597776
$script:CivicCastSignedGStreamerWorkflowRun = '45038334051'
$script:CivicCastSignedGStreamerStatus = 'Valid'
$script:CivicCastSignedGStreamerSignerSubject = 'CN=Scott Converse, O=Scott Converse, L=Longmont, S=co, C=US'
$script:CivicCastSignedGStreamerSignerThumb = '2DF3B3F01D2FEC69EF90CC941C26B21ED079349B'
$script:CivicCastSignedGStreamerTimestamp = 'CN=Microsoft Public RSA Time Stamping Authority, OU=nShield TSS ESN:7D00-05E0-D947, OU=Microsoft America Operations, O=Microsoft Corporation, L=Redmond, S=Washington, C=US'

function Get-CivicCastSignedGStreamerPin {
    [CmdletBinding()]
    param()
    $receiptPath = Join-Path $PSScriptRoot 'gstreamer-windows-signed.receipt.json'
    if (-not (Test-Path -LiteralPath $receiptPath -PathType Leaf)) {
        throw "Signed GStreamer receipt not found: $receiptPath"
    }
    $receipt = Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json
    if ([string]$receipt.file -cne $script:CivicCastSignedGStreamerReceiptFile) { throw "Receipt file mismatch: got '$($receipt.file)'" }
    if ([string]$receipt.signed_sha256 -cne $script:CivicCastSignedGStreamerSignedSha256) { throw 'Receipt signed_sha256 mismatch' }
    if ([string]$receipt.unsigned_sha256 -cne $script:CivicCastSignedGStreamerUnsignedSha256) { throw 'Receipt unsigned_sha256 mismatch' }
    if ([long]$receipt.signed_bytes -ne $script:CivicCastSignedGStreamerSignedBytes) { throw 'Receipt signed_bytes mismatch' }
    if ([string]$receipt.workflow_run -cne $script:CivicCastSignedGStreamerWorkflowRun) { throw 'Receipt workflow_run mismatch' }
    if ([string]$receipt.status -cne $script:CivicCastSignedGStreamerStatus) { throw 'Receipt status mismatch' }
    if ([string]$receipt.signer_subject -cne $script:CivicCastSignedGStreamerSignerSubject) { throw 'Receipt signer_subject mismatch' }
    if ([string]$receipt.signer_thumb -cne $script:CivicCastSignedGStreamerSignerThumb) { throw 'Receipt signer_thumb mismatch' }
    if ([string]$receipt.timestamp -cne $script:CivicCastSignedGStreamerTimestamp) { throw 'Receipt timestamp mismatch' }
    return [pscustomobject]@{
        file = $script:CivicCastSignedGStreamerFile
        url = $script:CivicCastSignedGStreamerUrl
        signed_sha256 = $script:CivicCastSignedGStreamerSignedSha256
        unsigned_sha256 = $script:CivicCastSignedGStreamerUnsignedSha256
        signed_bytes = $script:CivicCastSignedGStreamerSignedBytes
        workflow_run = $script:CivicCastSignedGStreamerWorkflowRun
        status = $script:CivicCastSignedGStreamerStatus
        signer_subject = $script:CivicCastSignedGStreamerSignerSubject
        signer_thumb = $script:CivicCastSignedGStreamerSignerThumb
        timestamp = $script:CivicCastSignedGStreamerTimestamp
    }
}

function Assert-CivicCastSignedGStreamer {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$LiteralPath
    )
    $pin = Get-CivicCastSignedGStreamerPin
    $full = [IO.Path]::GetFullPath($LiteralPath)
    if (-not (Test-Path -LiteralPath $full -PathType Leaf)) { throw "Signed GStreamer executable not found: $full" }
    $actualHash = (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualHash -cne $pin.signed_sha256) { throw "Signed GStreamer SHA256 mismatch: expected=$($pin.signed_sha256) actual=$actualHash" }
    $actualBytes = [long](Get-Item -LiteralPath $full).Length
    if ($actualBytes -ne $pin.signed_bytes) { throw "Signed GStreamer bytes mismatch: expected=$($pin.signed_bytes) actual=$actualBytes" }
    $sig = Get-AuthenticodeSignature -LiteralPath $full
    if ([string]$sig.Status -ne $pin.status) { throw "Signed GStreamer Authenticode status not '$($pin.status)': $($sig.Status)" }
    if ($null -eq $sig.SignerCertificate) { throw 'Signed GStreamer signature has no SignerCertificate' }
    if ([string]$sig.SignerCertificate.Subject -cne $pin.signer_subject) { throw "Signed GStreamer signer subject mismatch: got '$($sig.SignerCertificate.Subject)'" }
    if ([string]$sig.SignerCertificate.Thumbprint -cne $pin.signer_thumb) { throw "Signed GStreamer signer thumbprint mismatch: got '$($sig.SignerCertificate.Thumbprint)'" }
    if ($null -eq $sig.TimeStamperCertificate) { throw 'Signed GStreamer signature has no TimeStamperCertificate' }
    if ([string]$sig.TimeStamperCertificate.Subject -cne $pin.timestamp) { throw "Signed GStreamer timestamp subject mismatch: got '$($sig.TimeStamperCertificate.Subject)'" }
    return [pscustomobject]@{
        Path = $full
        SignedSha256 = $actualHash
        SignedBytes = $actualBytes
        Status = [string]$sig.Status
        SignerSubject = [string]$sig.SignerCertificate.Subject
        SignerThumbprint = [string]$sig.SignerCertificate.Thumbprint
        TimestampSubject = [string]$sig.TimeStamperCertificate.Subject
    }
}
