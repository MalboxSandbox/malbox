variable "name" {
  type        = string
  description = "Name of the VM/template to create"
}

variable "iso_url" {
  type        = string
  description = "URL or local path to the Windows ISO file"
}

variable "iso_checksum" {
  type        = string
  description = "Checksum of the ISO file"
}

variable "virtio_iso_path" {
  type        = string
  description = "Path to the VirtIO drivers ISO (virtio-win.iso)"
}

variable "disk_size" {
  type        = string
  default     = "61440M"
  description = "Disk size (e.g. 61440M)"
}

variable "cpus" {
  type        = number
  default     = 2
  description = "Number of CPUs"
}

variable "memory" {
  type        = number
  default     = 4096
  description = "Memory in MB"
}

variable "output_directory" {
  type        = string
  default     = "output"
  description = "Output directory for the built image"
}

variable "headless" {
  type        = bool
  default     = true
  description = "Run the build in headless mode"
}

variable "autounattend_path" {
  type        = string
  default     = "autounattend/Autounattend.xml"
  description = "Path to the Autounattend.xml file"
}

variable "tools_path" {
  type        = string
  default     = "C:\\Tools"
  description = "Path where analysis tools will be installed"
}

variable "winrm_username" {
  type        = string
  default     = "Administrator"
  description = "WinRM username for Packer communicator"
}

variable "winrm_password" {
  type        = string
  default     = "packer"
  sensitive   = true
  description = "WinRM password for Packer communicator"
}

variable "guest_plugin_path" {
  type        = string
  default     = ""
  description = "Path to a pre-built guest plugin .exe to bake into the image"
}

variable "guest_plugin_toml" {
  type        = string
  default     = ""
  description = "Path to the guest plugin's plugin.toml manifest"
}

source "qemu" "windows" {
  vm_name          = var.name
  output_directory = var.output_directory
  headless         = var.headless

  iso_url      = var.iso_url
  iso_checksum = var.iso_checksum

  cpus      = var.cpus
  memory    = var.memory
  disk_size = var.disk_size

  format      = "qcow2"
  accelerator = "kvm"

  disk_interface = "virtio"
  net_device     = "e1000"

  floppy_files = [
    var.autounattend_path,
    "scripts/enable-winrm.ps1",
  ]

  # Explicitly specify all drives to avoid Packer's qemuargs merging issues.
  # Pattern from github.com/StefanScherer/packer-windows
  qemuargs = [
    ["-drive", "file=${var.output_directory}/${var.name},if=virtio,cache=writeback,discard=ignore,format=qcow2,index=1"],
    ["-drive", "file=${var.iso_url},media=cdrom,index=2"],
    ["-drive", "file=${var.virtio_iso_path},media=cdrom,index=3"],
    ["-cpu", "host"],
    ["-device", "e1000,netdev=user.0"],
    ["-netdev", "user,id=user.0,hostfwd=tcp::{{ .SSHHostPort }}-:5986"],
  ]

  communicator   = "winrm"
  winrm_username = var.winrm_username
  winrm_password = var.winrm_password
  winrm_port     = 5986
  winrm_timeout  = "6h"
  winrm_use_ssl  = true
  winrm_insecure = true

  shutdown_command = "shutdown /s /t 10 /f /d p:4:1 /c \"Packer Shutdown\""
  shutdown_timeout = "15m"

  boot_wait = "6m"
}

build {
  sources = ["source.qemu.windows"]

  provisioner "powershell" {
    scripts = [
      // "scripts/disable-defender.ps1",
      "scripts/disable-updates.ps1",
      //"scripts/install-tools.ps1",
    ]
    environment_vars = [
      "TOOLS_PATH=${var.tools_path}",
    ]
  }

  provisioner "ansible" {
    playbook_file = "../../../ansible/playbooks/windows/windows.yml"
    use_proxy     = false
    user          = var.winrm_username
    extra_arguments = [
      "-e", "ansible_winrm_server_cert_validation=ignore",
      "-e", "ansible_password=${var.winrm_password}",
      "-e", "ansible_connection=winrm",
      "-e", "tools_path=${var.tools_path}",
    ]
  }
}
