# Set network to Private (WinRM refuses to enable on Public networks)
Get-NetConnectionProfile | Set-NetConnectionProfile -NetworkCategory Private

Enable-PSRemoting -Force
winrm quickconfig -q
winrm set winrm/config '@{MaxTimeoutms="1800000"}'
winrm set winrm/config/winrs '@{MaxMemoryPerShellMB="800"}'
winrm set winrm/config/service '@{AllowUnencrypted="false"}'
winrm set winrm/config/service/auth '@{Basic="true"}'
winrm set winrm/config/client/auth '@{Basic="true"}'

# Create self-signed certificate for HTTPS listener
$cert = New-SelfSignedCertificate -DnsName "packer" -CertStoreLocation Cert:\LocalMachine\My
winrm create winrm/config/listener?Address=*+Transport=HTTPS "@{CertificateThumbprint=`"$($cert.Thumbprint)`"}"

# Remove HTTP listener (only use HTTPS)
winrm delete winrm/config/listener?Address=*+Transport=HTTP 2>$null

# Firewall rule for WinRM HTTPS
netsh advfirewall firewall add rule name="WinRM HTTPS" dir=in action=allow protocol=TCP localport=5986

Set-Service winrm -StartupType "auto"
Restart-Service winrm
