variable "name" {
  type        = string
  description = "Name of the VM/template to create"
}

variable "iso_url" {
  type        = string
  description = "URL or local path to the Ubuntu Server ISO"
}

variable "iso_checksum" {
  type        = string
  description = "Checksum of the ISO file"
}

variable "disk_size" {
  type        = string
  default     = "40960M"
  description = "Disk size (e.g. 40960M)"
}

variable "cpus" {
  type        = number
  default     = 2
  description = "Number of CPUs"
}

variable "memory" {
  type        = number
  default     = 2048
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

variable "ssh_username" {
  type        = string
  default     = "malbox"
  description = "SSH username for Packer communicator"
}

variable "ssh_password" {
  type        = string
  default     = "packer"
  sensitive   = true
  description = "SSH password for Packer communicator"
}

source "qemu" "linux" {
  vm_name          = var.name
  output_directory = var.output_directory
  headless         = var.headless

  iso_url      = var.iso_url
  iso_checksum = var.iso_checksum

  cpus      = var.cpus
  memory    = var.memory
  disk_size = var.disk_size

  format       = "qcow2"
  accelerator  = "kvm"
  machine_type = "q35"

  disk_interface = "virtio"
  net_device     = "virtio-net"

  qemuargs = [
    ["-cpu", "host"],
  ]

  # Serve cloud-init autoinstall via HTTP
  http_directory = "http"

  boot_command = [
    "<spacebar><wait><spacebar><wait><spacebar><wait><spacebar><wait><spacebar><wait>",
    "e<wait>",
    "<down><down><down><end>",
    " autoinstall ds=nocloud-net\\;s=http://{{ .HTTPIP }}:{{ .HTTPPort }}/",
    "<f10>",
  ]
  boot_wait = "5s"

  communicator = "ssh"
  ssh_username = var.ssh_username
  ssh_password = var.ssh_password
  ssh_timeout  = "30m"

  shutdown_command = "echo '${var.ssh_password}' | sudo -S shutdown -P now"
  shutdown_timeout = "10m"
}

build {
  sources = ["source.qemu.linux"]

  provisioner "shell" {
    inline = [
      "sudo apt-get update",
      "sudo apt-get upgrade -y",
      "sudo apt-get install -y python3 python3-pip qemu-guest-agent",
      "sudo systemctl enable qemu-guest-agent",
    ]
  }

  provisioner "ansible" {
    playbook_file = "../../../ansible/playbooks/linux/base.yml"
    user          = var.ssh_username
    extra_arguments = [
      "-e", "ansible_become_pass=${var.ssh_password}",
    ]
  }
}
