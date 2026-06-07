#!/usr/bin/env bash
# Written in [Amber](https://amber-lang.com/)
# version: 0.6.0-alpha
if [ -n "$ZSH_VERSION" ]; then
    EXEC_SHELL="zsh"
    IFS='.' read -A EXEC_SHELL_VERSION <<< "$ZSH_VERSION"
elif [ -n "$KSH_VERSION" ]; then
    EXEC_SHELL="ksh"
    __exec_shell_version="${.sh.version##*/}"
    IFS='.' read -a EXEC_SHELL_VERSION <<< "${__exec_shell_version%% *}"
else
    EXEC_SHELL="bash"
    EXEC_SHELL_VERSION=("${BASH_VERSINFO[0]}" "${BASH_VERSINFO[1]}" "${BASH_VERSINFO[2]}")
fi
# split(text: Text, delimiter: Text)
split__4_v0() {
    local text_45="${1}"
    local delimiter_46="${2}"
    local result_47=()
    # zsh uses -A for array, bash uses -a, ksh is VERY bad at splitting anything
    if [ "$([ "_${EXEC_SHELL}" != "_zsh" ]; echo $?)" != 0 ]; then
        IFS="${delimiter_46}" read -rd '' -A result_47 < <(printf %s "$text_45")
        __status=$?
    elif [ "$([ "_${EXEC_SHELL}" != "_ksh" ]; echo $?)" != 0 ]; then
        if [ "$([ "_${delimiter_46}" != "_
" ]; echo $?)" != 0 ]; then
            while read -r -d $'\n'; do result_47+=("$REPLY"); done < <(echo "$text_45")
            __status=$?
        else
            IFS="${delimiter_46}" read -rd '' -a result_47 < <(printf %s "$text_45")
            __status=$?
        fi
    elif [ "$([ "_${EXEC_SHELL}" != "_bash" ]; echo $?)" != 0 ]; then
        IFS="${delimiter_46}" read -rd '' -a result_47 < <(printf %s "$text_45")
        __status=$?
    fi
    ret_split4_v0=("${result_47[@]}")
    return 0
}

__GITHUB_REPO_3="DualHorizon/malbox"
__BINARY_NAME_4="malboxctl"
# Pinned so a curl|bash bootstrap never pulls a moving, unreviewed dependency.
__GUM_VERSION_5="0.17.0"
__GUM_TMP_DIR_6=""
__COLOR_ACCENT_7="#516cf9"
__COLOR_SUCCESS_8="#4ade80"
__COLOR_ERROR_9="#ef4444"
__COLOR_WARN_10="#f59e0b"
__COLOR_FAINT_11="#8a8f94"
# ensure_gum()
ensure_gum__37_v0() {
    local command_1
    command_1="$(command -v gum > /dev/null 2>&1 && echo "yes" || echo "no")"
    __status=$?
    local has_gum_20="${command_1}"
    if [ "$([ "_${has_gum_20}" != "_no" ]; echo $?)" != 0 ]; then
        local command_2
        command_2="$(uname -m)"
        __status=$?
        local uname_arch_21="${command_2}"
        local gum_arch_22=""
        if [ "$(( $([ "_${uname_arch_21}" != "_x86_64" ]; echo $?) || $([ "_${uname_arch_21}" != "_amd64" ]; echo $?) ))" != 0 ]; then
            gum_arch_22="x86_64"
        elif [ "$(( $([ "_${uname_arch_21}" != "_aarch64" ]; echo $?) || $([ "_${uname_arch_21}" != "_arm64" ]; echo $?) ))" != 0 ]; then
            gum_arch_22="arm64"
        else
            echo "Error: unsupported architecture for gum: ${uname_arch_21}"
            exit 1
        fi
        local gum_asset_23="gum_${__GUM_VERSION_5}_Linux_${gum_arch_22}.tar.gz"
        local gum_url_24="https://github.com/charmbracelet/gum/releases/download/v${__GUM_VERSION_5}/${gum_asset_23}"
        local command_3
        command_3="$(mktemp -d)"
        __status=$?
        __GUM_TMP_DIR_6="${command_3}"
        curl -sSfL "${gum_url_24}" | tar -xz -C "${__GUM_TMP_DIR_6}" --strip-components=1 --wildcards "*/gum"
        __status=$?
        if [ "${__status}" != 0 ]; then
            echo "Error: could not download gum"
            rm -rf "${__GUM_TMP_DIR_6}">/dev/null 2>&1
            __status=$?
            exit 1
        fi
        export PATH="${__GUM_TMP_DIR_6}:$PATH"
        __status=$?
    fi
}

# cleanup_gum()
cleanup_gum__38_v0() {
    if [ "$([ "_${__GUM_TMP_DIR_6}" == "_" ]; echo $?)" != 0 ]; then
        rm -rf "${__GUM_TMP_DIR_6}">/dev/null 2>&1
        __status=$?
    fi
}

# check_os()
check_os__39_v0() {
    local command_4
    command_4="$(uname -s)"
    __status=$?
    local os_14="${command_4}"
    if [ "$([ "_${os_14}" == "_Linux" ]; echo $?)" != 0 ]; then
        gum log --level error --prefix.foreground "${__COLOR_ERROR_9}" "Malbox currently only supports Linux (got ${os_14})"
        __status=$?
        exit 1
    fi
    ret_check_os39_v0="${os_14}"
    return 0
}

# detect_arch()
detect_arch__40_v0() {
    local command_5
    command_5="$(uname -m)"
    __status=$?
    local uname_arch_26="${command_5}"
    if [ "$(( $([ "_${uname_arch_26}" != "_x86_64" ]; echo $?) || $([ "_${uname_arch_26}" != "_amd64" ]; echo $?) ))" != 0 ]; then
        ret_detect_arch40_v0="linux-x64"
        return 0
    elif [ "$(( $([ "_${uname_arch_26}" != "_aarch64" ]; echo $?) || $([ "_${uname_arch_26}" != "_arm64" ]; echo $?) ))" != 0 ]; then
        ret_detect_arch40_v0="linux-arm64"
        return 0
    else
        gum log --level error --prefix.foreground "${__COLOR_ERROR_9}" "Unsupported architecture: ${uname_arch_26}"
        __status=$?
        cleanup_gum__38_v0 
        exit 1
    fi
    ret_detect_arch40_v0=""
    return 0
}

# detect_install_dir()
detect_install_dir__41_v0() {
    local command_6
    command_6="$(id -u)"
    __status=$?
    local uid_41="${command_6}"
    if [ "$([ "_${uid_41}" != "_0" ]; echo $?)" != 0 ]; then
        ret_detect_install_dir41_v0="/usr/local/bin"
        return 0
    fi
    local command_7
    command_7="$(echo $HOME)"
    __status=$?
    local home_42="${command_7}"
    local local_bin_43="${home_42}/.local/bin"
    local command_8
    command_8="$(echo $PATH)"
    __status=$?
    local path_var_44="${command_8}"
    split__4_v0 "${path_var_44}" ":"
    local path_entries_48=("${ret_split4_v0[@]}")
    local found_49=0
    for entry_50 in "${path_entries_48[@]}"; do
        if [ "$([ "_${entry_50}" != "_${local_bin_43}" ]; echo $?)" != 0 ]; then
            found_49=1
            break
        fi
    done
    if [ "$(( ! found_49 ))" != 0 ]; then
        gum log --level warn --prefix.foreground "${__COLOR_WARN_10}" "${local_bin_43} is not on your PATH"
        __status=$?
        gum style --foreground "${__COLOR_FAINT_11}" "Add it with: export PATH=\"$HOME/.local/bin:$PATH\""
        __status=$?
        printf '%s\n' ""
    fi
    mkdir -p "${local_bin_43}">/dev/null 2>&1
    __status=$?
    ret_detect_install_dir41_v0="${local_bin_43}"
    return 0
}

# choose_channel()
choose_channel__42_v0() {
    local command_11
    command_11="$(gum choose --cursor.foreground "${__COLOR_ACCENT_7}" --selected.foreground "${__COLOR_ACCENT_7}" --header.foreground "${__COLOR_FAINT_11}" "stable" "nightly")"
    __status=$?
    if [ "${__status}" != 0 ]; then
        gum log --level error --prefix.foreground "${__COLOR_ERROR_9}" "No channel selected"
        __status=$?
        cleanup_gum__38_v0 
        exit 1
    fi
    local channel_53="${command_11}"
    ret_choose_channel42_v0="${channel_53}"
    return 0
}

# fetch_release(channel: Text)
fetch_release__43_v0() {
    local channel_60="${1}"
    # We always list `/releases` (newest-first) rather than `/releases/latest`,
    # because the latter only returns non-prerelease releases and malbox
    # currently ships everything as a prerelease. "nightly" takes the newest
    # release of any kind; "stable" takes the newest release whose tag has no
    # pre-release suffix (e.g. v0.1.0 but not v0.1.0-alpha.5).
    local command_12
    command_12="$(gum spin --spinner dot --spinner.foreground "${__COLOR_ACCENT_7}" --title "Fetching latest ${channel_60} release..." --show-output -- curl -sSfL "https://api.github.com/repos/${__GITHUB_REPO_3}/releases")"
    __status=$?
    if [ "${__status}" != 0 ]; then
        gum log --level error --prefix.foreground "${__COLOR_ERROR_9}" "Could not fetch release information"
        __status=$?
        cleanup_gum__38_v0 
        exit 1
    fi
    local releases_json_61="${command_12}"
    local command_13
    command_13="$(printf "%s" "${releases_json_61}" | grep "tag_name" | sed 's/.*: *"//;s/".*//')"
    __status=$?
    local tags_62="${command_13}"
    local tag_63=""
    if [ "$([ "_${channel_60}" != "_stable" ]; echo $?)" != 0 ]; then
        local command_14
        command_14="$(printf "%s" "${tags_62}" | grep -v -- "-" | head -1)"
        __status=$?
        tag_63="${command_14}"
    else
        local command_15
        command_15="$(printf "%s" "${tags_62}" | head -1)"
        __status=$?
        tag_63="${command_15}"
    fi
    if [ "$([ "_${tag_63}" != "_" ]; echo $?)" != 0 ]; then
        if [ "$([ "_${channel_60}" != "_stable" ]; echo $?)" != 0 ]; then
            gum log --level error --prefix.foreground "${__COLOR_ERROR_9}" "No stable release available yet (only pre-releases exist - try the nightly channel)"
            __status=$?
        else
            gum log --level error --prefix.foreground "${__COLOR_ERROR_9}" "Could not determine latest ${channel_60} release"
            __status=$?
        fi
        cleanup_gum__38_v0 
        exit 1
    fi
    gum log --level info --prefix.foreground "${__COLOR_ACCENT_7}" "Latest ${channel_60}: ${tag_63}"
    __status=$?
    ret_fetch_release43_v0="${tag_63}"
    return 0
}

# download_binary(tag: Text, asset_name: Text)
download_binary__44_v0() {
    local tag_71="${1}"
    local asset_name_72="${2}"
    local download_url_73="https://github.com/${__GITHUB_REPO_3}/releases/download/${tag_71}/${asset_name_72}"
    local checksum_url_74="${download_url_73}.sha256"
    local command_16
    command_16="$(mktemp -d)"
    __status=$?
    local tmp_dir_75="${command_16}"
    gum spin --spinner dot --spinner.foreground "${__COLOR_ACCENT_7}" --title "Downloading ${asset_name_72}..." -- curl -sSfL -o "${tmp_dir_75}/${asset_name_72}" "${download_url_73}"
    __status=$?
    if [ "${__status}" != 0 ]; then
        gum log --level error --prefix.foreground "${__COLOR_ERROR_9}" "Failed to download binary"
        __status=$?
        gum style --foreground "${__COLOR_FAINT_11}" "No prebuilt binary may exist yet for this platform (${asset_name_72})."
        __status=$?
        gum style --foreground "${__COLOR_FAINT_11}" "Check https://github.com/${__GITHUB_REPO_3}/releases for available assets."
        __status=$?
        rm -rf "${tmp_dir_75}">/dev/null 2>&1
        __status=$?
        cleanup_gum__38_v0 
        exit 1
    fi
    curl -sSfL -o "${tmp_dir_75}/${asset_name_72}.sha256" "${checksum_url_74}"
    __status=$?
    if [ "${__status}" != 0 ]; then
        gum log --level warn --prefix.foreground "${__COLOR_WARN_10}" "No checksum available, skipping verification"
        __status=$?
    fi
    ret_download_binary44_v0="${tmp_dir_75}"
    return 0
}

# verify_checksum(tmp_dir: Text, asset_name: Text)
verify_checksum__45_v0() {
    local tmp_dir_81="${1}"
    local asset_name_82="${2}"
    # When no sidecar was published the download step already warned; only a
    # present-but-mismatching checksum is a hard failure (corrupted or
    # tampered download, never something to install).
    local command_17
    command_17="$(test -f "${tmp_dir_81}/${asset_name_82}.sha256" && echo "yes" || echo "no")"
    __status=$?
    local has_checksum_83="${command_17}"
    if [ "$([ "_${has_checksum_83}" != "_yes" ]; echo $?)" != 0 ]; then
        local verified_84=0
        gum spin --spinner dot --spinner.foreground "${__COLOR_ACCENT_7}" --title "Verifying checksum..." -- bash -c "cd \"${tmp_dir_81}\" && sha256sum -c \"${asset_name_82}.sha256\""
        __status=$?
        if [ "${__status}" = 0 ]; then
            verified_84=1
        fi
        if [ "$(( ! verified_84 ))" != 0 ]; then
            bash -c "cd \"${tmp_dir_81}\" && shasum -a 256 -c \"${asset_name_82}.sha256\""
            __status=$?
            if [ "${__status}" = 0 ]; then
                verified_84=1
            fi
        fi
        if [ "$(( ! verified_84 ))" != 0 ]; then
            gum log --level error --prefix.foreground "${__COLOR_ERROR_9}" "Checksum verification failed - aborting installation"
            __status=$?
            rm -rf "${tmp_dir_81}">/dev/null 2>&1
            __status=$?
            cleanup_gum__38_v0 
            exit 1
        fi
    fi
}

# install_binary(tmp_dir: Text, asset_name: Text, install_dir: Text)
install_binary__46_v0() {
    local tmp_dir_88="${1}"
    local asset_name_89="${2}"
    local install_dir_90="${3}"
    tar -xzf "${tmp_dir_88}/${asset_name_89}" -C "${tmp_dir_88}" --strip-components=1
    __status=$?
    if [ "${__status}" != 0 ]; then
        gum log --level error --prefix.foreground "${__COLOR_ERROR_9}" "Failed to extract archive"
        __status=$?
        rm -rf "${tmp_dir_88}">/dev/null 2>&1
        __status=$?
        cleanup_gum__38_v0 
        exit 1
    fi
    mv "${tmp_dir_88}/malboxctl" "${install_dir_90}/${__BINARY_NAME_4}"
    __status=$?
    if [ "${__status}" != 0 ]; then
        gum log --level error --prefix.foreground "${__COLOR_ERROR_9}" "Failed to install binary"
        __status=$?
        rm -rf "${tmp_dir_88}">/dev/null 2>&1
        __status=$?
        cleanup_gum__38_v0 
        exit 1
    fi
    chmod +x "${install_dir_90}/${__BINARY_NAME_4}"
    __status=$?
    if [ "${__status}" != 0 ]; then
        gum log --level error --prefix.foreground "${__COLOR_ERROR_9}" "Failed to set permissions"
        __status=$?
        rm -rf "${tmp_dir_88}">/dev/null 2>&1
        __status=$?
        cleanup_gum__38_v0 
        exit 1
    fi
    rm -rf "${tmp_dir_88}">/dev/null 2>&1
    __status=$?
}

check_os__39_v0 
ensure_gum__37_v0 
detect_arch__40_v0 
arch_27="${ret_detect_arch40_v0}"
detect_install_dir__41_v0 
install_dir_51="${ret_detect_install_dir41_v0}"
gum style --border rounded --border-foreground "${__COLOR_ACCENT_7}" --padding "0 2" --bold "Installing Malbox CLI"
__status=$?
printf '%s\n' ""
gum log --level info --prefix.foreground "${__COLOR_ACCENT_7}" "Architecture: ${arch_27}"
__status=$?
gum log --level info --prefix.foreground "${__COLOR_ACCENT_7}" "Install to:   ${install_dir_51}/${__BINARY_NAME_4}"
__status=$?
printf '%s\n' ""
gum style --bold "Select release channel:"
__status=$?
choose_channel__42_v0 
channel_54="${ret_choose_channel42_v0}"
printf '%s\n' ""
test -f "${install_dir_51}/${__BINARY_NAME_4}"
__status=$?
if [ "${__status}" = 0 ]; then
    command_19="$(printenv MALBOX_FORCE || true)"
    __status=$?
    force_55="${command_19}"
    if [ "$([ "_${force_55}" == "_1" ]; echo $?)" != 0 ]; then
        gum confirm --prompt.foreground "${__COLOR_ACCENT_7}" --selected.background "${__COLOR_ACCENT_7}" --unselected.foreground "${__COLOR_FAINT_11}" "Malbox CLI already exists at ${install_dir_51}/${__BINARY_NAME_4}. Overwrite?"
        __status=$?
        if [ "${__status}" != 0 ]; then
            gum log --level info --prefix.foreground "${__COLOR_ACCENT_7}" "Installation cancelled."
            __status=$?
            cleanup_gum__38_v0 
            exit 0
        fi
    fi
fi
fetch_release__43_v0 "${channel_54}"
tag_64="${ret_fetch_release43_v0}"
asset_name_65="malboxctl-${tag_64}-${arch_27}.tar.gz"
download_binary__44_v0 "${tag_64}" "${asset_name_65}"
tmp_dir_76="${ret_download_binary44_v0}"
verify_checksum__45_v0 "${tmp_dir_76}" "${asset_name_65}"
install_binary__46_v0 "${tmp_dir_76}" "${asset_name_65}" "${install_dir_51}"
printf '%s\n' ""
gum style --foreground "${__COLOR_SUCCESS_8}" --bold "Malbox CLI installed successfully!"
__status=$?
gum style --foreground "${__COLOR_FAINT_11}" "Run malboxctl install to set up Malbox."
__status=$?
cleanup_gum__38_v0 
