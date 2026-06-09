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
    local text_46="${1}"
    local delimiter_47="${2}"
    local result_48=()
    # zsh uses -A for array, bash uses -a, ksh is VERY bad at splitting anything
    if [ "$([ "_${EXEC_SHELL}" != "_zsh" ]; echo $?)" != 0 ]; then
        IFS="${delimiter_47}" read -rd '' -A result_48 < <(printf %s "$text_46")
        __status=$?
    elif [ "$([ "_${EXEC_SHELL}" != "_ksh" ]; echo $?)" != 0 ]; then
        if [ "$([ "_${delimiter_47}" != "_
" ]; echo $?)" != 0 ]; then
            while read -r -d $'\n'; do result_48+=("$REPLY"); done < <(echo "$text_46")
            __status=$?
        else
            IFS="${delimiter_47}" read -rd '' -a result_48 < <(printf %s "$text_46")
            __status=$?
        fi
    elif [ "$([ "_${EXEC_SHELL}" != "_bash" ]; echo $?)" != 0 ]; then
        IFS="${delimiter_47}" read -rd '' -a result_48 < <(printf %s "$text_46")
        __status=$?
    fi
    ret_split4_v0=("${result_48[@]}")
    return 0
}

__GITHUB_REPO_3="DualHorizon/malbox"
__BINARY_NAME_4="malboxctl"
# Channel used for non-interactive installs (curl|bash with no MALBOX_CHANNEL).
__DEFAULT_CHANNEL_5="stable"
# Pinned so a curl|bash bootstrap never pulls a moving, unreviewed dependency.
__GUM_VERSION_6="0.17.0"
__GUM_TMP_DIR_7=""
__COLOR_ACCENT_8="#516cf9"
__COLOR_SUCCESS_9="#4ade80"
__COLOR_ERROR_10="#ef4444"
__COLOR_WARN_11="#f59e0b"
__COLOR_FAINT_12="#8a8f94"
# ensure_gum()
ensure_gum__37_v0() {
    local command_1
    command_1="$(command -v gum > /dev/null 2>&1 && echo "yes" || echo "no")"
    __status=$?
    local has_gum_21="${command_1}"
    if [ "$([ "_${has_gum_21}" != "_no" ]; echo $?)" != 0 ]; then
        local command_2
        command_2="$(uname -m)"
        __status=$?
        local uname_arch_22="${command_2}"
        local gum_arch_23=""
        if [ "$(( $([ "_${uname_arch_22}" != "_x86_64" ]; echo $?) || $([ "_${uname_arch_22}" != "_amd64" ]; echo $?) ))" != 0 ]; then
            gum_arch_23="x86_64"
        elif [ "$(( $([ "_${uname_arch_22}" != "_aarch64" ]; echo $?) || $([ "_${uname_arch_22}" != "_arm64" ]; echo $?) ))" != 0 ]; then
            gum_arch_23="arm64"
        else
            echo "Error: unsupported architecture for gum: ${uname_arch_22}"
            exit 1
        fi
        local gum_asset_24="gum_${__GUM_VERSION_6}_Linux_${gum_arch_23}.tar.gz"
        local gum_url_25="https://github.com/charmbracelet/gum/releases/download/v${__GUM_VERSION_6}/${gum_asset_24}"
        local command_3
        command_3="$(mktemp -d)"
        __status=$?
        __GUM_TMP_DIR_7="${command_3}"
        curl -sSfL "${gum_url_25}" | tar -xz -C "${__GUM_TMP_DIR_7}" --strip-components=1 --wildcards "*/gum"
        __status=$?
        if [ "${__status}" != 0 ]; then
            echo "Error: could not download gum"
            rm -rf "${__GUM_TMP_DIR_7}">/dev/null 2>&1
            __status=$?
            exit 1
        fi
        export PATH="${__GUM_TMP_DIR_7}:$PATH"
        __status=$?
    fi
}

# cleanup_gum()
cleanup_gum__38_v0() {
    if [ "$([ "_${__GUM_TMP_DIR_7}" == "_" ]; echo $?)" != 0 ]; then
        rm -rf "${__GUM_TMP_DIR_7}">/dev/null 2>&1
        __status=$?
    fi
}

# check_os()
check_os__39_v0() {
    local command_4
    command_4="$(uname -s)"
    __status=$?
    local os_15="${command_4}"
    if [ "$([ "_${os_15}" == "_Linux" ]; echo $?)" != 0 ]; then
        gum log --level error --prefix.foreground "${__COLOR_ERROR_10}" "Malbox currently only supports Linux (got ${os_15})"
        __status=$?
        exit 1
    fi
    ret_check_os39_v0="${os_15}"
    return 0
}

# detect_arch()
detect_arch__40_v0() {
    local command_5
    command_5="$(uname -m)"
    __status=$?
    local uname_arch_27="${command_5}"
    if [ "$(( $([ "_${uname_arch_27}" != "_x86_64" ]; echo $?) || $([ "_${uname_arch_27}" != "_amd64" ]; echo $?) ))" != 0 ]; then
        ret_detect_arch40_v0="linux-x64"
        return 0
    elif [ "$(( $([ "_${uname_arch_27}" != "_aarch64" ]; echo $?) || $([ "_${uname_arch_27}" != "_arm64" ]; echo $?) ))" != 0 ]; then
        ret_detect_arch40_v0="linux-arm64"
        return 0
    else
        gum log --level error --prefix.foreground "${__COLOR_ERROR_10}" "Unsupported architecture: ${uname_arch_27}"
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
    local uid_42="${command_6}"
    if [ "$([ "_${uid_42}" != "_0" ]; echo $?)" != 0 ]; then
        ret_detect_install_dir41_v0="/usr/local/bin"
        return 0
    fi
    local command_7
    command_7="$(echo $HOME)"
    __status=$?
    local home_43="${command_7}"
    local local_bin_44="${home_43}/.local/bin"
    local command_8
    command_8="$(echo $PATH)"
    __status=$?
    local path_var_45="${command_8}"
    split__4_v0 "${path_var_45}" ":"
    local path_entries_49=("${ret_split4_v0[@]}")
    local found_50=0
    for entry_51 in "${path_entries_49[@]}"; do
        if [ "$([ "_${entry_51}" != "_${local_bin_44}" ]; echo $?)" != 0 ]; then
            found_50=1
            break
        fi
    done
    if [ "$(( ! found_50 ))" != 0 ]; then
        gum log --level warn --prefix.foreground "${__COLOR_WARN_11}" "${local_bin_44} is not on your PATH"
        __status=$?
        gum style --foreground "${__COLOR_FAINT_12}" "Add it with: export PATH=\"$HOME/.local/bin:$PATH\""
        __status=$?
        printf '%s\n' ""
    fi
    mkdir -p "${local_bin_44}">/dev/null 2>&1
    __status=$?
    ret_detect_install_dir41_v0="${local_bin_44}"
    return 0
}

# is_interactive()
is_interactive__42_v0() {
    # curl|bash leaves stdin as the pipe, not a tty; gum prompts can't read it.
    local command_11
    command_11="$(test -t 0 && echo "yes" || echo "no")"
    __status=$?
    ret_is_interactive42_v0="${command_11}"
    return 0
}

# choose_channel()
choose_channel__43_v0() {
    # An explicit MALBOX_CHANNEL wins everywhere, interactive or not.
    local command_12
    command_12="$(printenv MALBOX_CHANNEL || true)"
    __status=$?
    local env_channel_56="${command_12}"
    if [ "$([ "_${env_channel_56}" == "_" ]; echo $?)" != 0 ]; then
        if [ "$(( $([ "_${env_channel_56}" == "_stable" ]; echo $?) && $([ "_${env_channel_56}" == "_nightly" ]; echo $?) ))" != 0 ]; then
            gum log --level error --prefix.foreground "${__COLOR_ERROR_10}" "Invalid MALBOX_CHANNEL: ${env_channel_56} (expected 'stable' or 'nightly')"
            __status=$?
            cleanup_gum__38_v0 
            exit 1
        fi
        ret_choose_channel43_v0="${env_channel_56}"
        return 0
    fi
    is_interactive__42_v0 
    local interactive_57="${ret_is_interactive42_v0}"
    if [ "$([ "_${interactive_57}" != "_no" ]; echo $?)" != 0 ]; then
        gum log --level info --prefix.foreground "${__COLOR_ACCENT_8}" "Non-interactive install, defaulting to ${__DEFAULT_CHANNEL_5} channel (set MALBOX_CHANNEL to override)"
        __status=$?
        ret_choose_channel43_v0="${__DEFAULT_CHANNEL_5}"
        return 0
    fi
    gum style --bold "Select release channel:"
    __status=$?
    local command_13
    command_13="$(gum choose --cursor.foreground "${__COLOR_ACCENT_8}" --selected.foreground "${__COLOR_ACCENT_8}" --header.foreground "${__COLOR_FAINT_12}" "stable" "nightly")"
    __status=$?
    if [ "${__status}" != 0 ]; then
        gum log --level error --prefix.foreground "${__COLOR_ERROR_10}" "No channel selected"
        __status=$?
        cleanup_gum__38_v0 
        exit 1
    fi
    local channel_58="${command_13}"
    ret_choose_channel43_v0="${channel_58}"
    return 0
}

# fetch_release(channel: Text)
fetch_release__44_v0() {
    local channel_66="${1}"
    # We always list `/releases` (newest-first) rather than `/releases/latest`,
    # because the latter only returns non-prerelease releases and malbox
    # currently ships everything as a prerelease. "nightly" takes the newest
    # release of any kind; "stable" takes the newest release whose tag has no
    # pre-release suffix (e.g. v0.1.0 but not v0.1.0-alpha.5).
    local command_14
    command_14="$(gum spin --spinner dot --spinner.foreground "${__COLOR_ACCENT_8}" --title "Fetching latest ${channel_66} release..." --show-output -- curl -sSfL "https://api.github.com/repos/${__GITHUB_REPO_3}/releases")"
    __status=$?
    if [ "${__status}" != 0 ]; then
        gum log --level error --prefix.foreground "${__COLOR_ERROR_10}" "Could not fetch release information"
        __status=$?
        cleanup_gum__38_v0 
        exit 1
    fi
    local releases_json_67="${command_14}"
    local command_15
    command_15="$(printf "%s" "${releases_json_67}" | grep "tag_name" | sed 's/.*: *"//;s/".*//')"
    __status=$?
    local tags_68="${command_15}"
    local tag_69=""
    if [ "$([ "_${channel_66}" != "_stable" ]; echo $?)" != 0 ]; then
        local command_16
        command_16="$(printf "%s" "${tags_68}" | grep -v -- "-" | head -1)"
        __status=$?
        tag_69="${command_16}"
    else
        local command_17
        command_17="$(printf "%s" "${tags_68}" | head -1)"
        __status=$?
        tag_69="${command_17}"
    fi
    if [ "$([ "_${tag_69}" != "_" ]; echo $?)" != 0 ]; then
        if [ "$([ "_${channel_66}" != "_stable" ]; echo $?)" != 0 ]; then
            gum log --level error --prefix.foreground "${__COLOR_ERROR_10}" "No stable release available yet (only pre-releases exist - try the nightly channel)"
            __status=$?
        else
            gum log --level error --prefix.foreground "${__COLOR_ERROR_10}" "Could not determine latest ${channel_66} release"
            __status=$?
        fi
        cleanup_gum__38_v0 
        exit 1
    fi
    gum log --level info --prefix.foreground "${__COLOR_ACCENT_8}" "Latest ${channel_66}: ${tag_69}"
    __status=$?
    ret_fetch_release44_v0="${tag_69}"
    return 0
}

# download_binary(tag: Text, asset_name: Text)
download_binary__45_v0() {
    local tag_77="${1}"
    local asset_name_78="${2}"
    local download_url_79="https://github.com/${__GITHUB_REPO_3}/releases/download/${tag_77}/${asset_name_78}"
    local checksum_url_80="${download_url_79}.sha256"
    local command_18
    command_18="$(mktemp -d)"
    __status=$?
    local tmp_dir_81="${command_18}"
    gum spin --spinner dot --spinner.foreground "${__COLOR_ACCENT_8}" --title "Downloading ${asset_name_78}..." -- curl -sSfL -o "${tmp_dir_81}/${asset_name_78}" "${download_url_79}"
    __status=$?
    if [ "${__status}" != 0 ]; then
        gum log --level error --prefix.foreground "${__COLOR_ERROR_10}" "Failed to download binary"
        __status=$?
        gum style --foreground "${__COLOR_FAINT_12}" "No prebuilt binary may exist yet for this platform (${asset_name_78})."
        __status=$?
        gum style --foreground "${__COLOR_FAINT_12}" "Check https://github.com/${__GITHUB_REPO_3}/releases for available assets."
        __status=$?
        rm -rf "${tmp_dir_81}">/dev/null 2>&1
        __status=$?
        cleanup_gum__38_v0 
        exit 1
    fi
    curl -sSfL -o "${tmp_dir_81}/${asset_name_78}.sha256" "${checksum_url_80}"
    __status=$?
    if [ "${__status}" != 0 ]; then
        gum log --level warn --prefix.foreground "${__COLOR_WARN_11}" "No checksum available, skipping verification"
        __status=$?
    fi
    ret_download_binary45_v0="${tmp_dir_81}"
    return 0
}

# verify_checksum(tmp_dir: Text, asset_name: Text)
verify_checksum__46_v0() {
    local tmp_dir_87="${1}"
    local asset_name_88="${2}"
    # When no sidecar was published the download step already warned; only a
    # present-but-mismatching checksum is a hard failure (corrupted or
    # tampered download, never something to install).
    local command_19
    command_19="$(test -f "${tmp_dir_87}/${asset_name_88}.sha256" && echo "yes" || echo "no")"
    __status=$?
    local has_checksum_89="${command_19}"
    if [ "$([ "_${has_checksum_89}" != "_yes" ]; echo $?)" != 0 ]; then
        local verified_90=0
        gum spin --spinner dot --spinner.foreground "${__COLOR_ACCENT_8}" --title "Verifying checksum..." -- bash -c "cd \"${tmp_dir_87}\" && sha256sum -c \"${asset_name_88}.sha256\""
        __status=$?
        if [ "${__status}" = 0 ]; then
            verified_90=1
        fi
        if [ "$(( ! verified_90 ))" != 0 ]; then
            bash -c "cd \"${tmp_dir_87}\" && shasum -a 256 -c \"${asset_name_88}.sha256\""
            __status=$?
            if [ "${__status}" = 0 ]; then
                verified_90=1
            fi
        fi
        if [ "$(( ! verified_90 ))" != 0 ]; then
            gum log --level error --prefix.foreground "${__COLOR_ERROR_10}" "Checksum verification failed - aborting installation"
            __status=$?
            rm -rf "${tmp_dir_87}">/dev/null 2>&1
            __status=$?
            cleanup_gum__38_v0 
            exit 1
        fi
    fi
}

# install_binary(tmp_dir: Text, asset_name: Text, install_dir: Text)
install_binary__47_v0() {
    local tmp_dir_94="${1}"
    local asset_name_95="${2}"
    local install_dir_96="${3}"
    tar -xzf "${tmp_dir_94}/${asset_name_95}" -C "${tmp_dir_94}" --strip-components=1
    __status=$?
    if [ "${__status}" != 0 ]; then
        gum log --level error --prefix.foreground "${__COLOR_ERROR_10}" "Failed to extract archive"
        __status=$?
        rm -rf "${tmp_dir_94}">/dev/null 2>&1
        __status=$?
        cleanup_gum__38_v0 
        exit 1
    fi
    mv "${tmp_dir_94}/malboxctl" "${install_dir_96}/${__BINARY_NAME_4}"
    __status=$?
    if [ "${__status}" != 0 ]; then
        gum log --level error --prefix.foreground "${__COLOR_ERROR_10}" "Failed to install binary"
        __status=$?
        rm -rf "${tmp_dir_94}">/dev/null 2>&1
        __status=$?
        cleanup_gum__38_v0 
        exit 1
    fi
    chmod +x "${install_dir_96}/${__BINARY_NAME_4}"
    __status=$?
    if [ "${__status}" != 0 ]; then
        gum log --level error --prefix.foreground "${__COLOR_ERROR_10}" "Failed to set permissions"
        __status=$?
        rm -rf "${tmp_dir_94}">/dev/null 2>&1
        __status=$?
        cleanup_gum__38_v0 
        exit 1
    fi
    rm -rf "${tmp_dir_94}">/dev/null 2>&1
    __status=$?
}

check_os__39_v0 
ensure_gum__37_v0 
detect_arch__40_v0 
arch_28="${ret_detect_arch40_v0}"
detect_install_dir__41_v0 
install_dir_52="${ret_detect_install_dir41_v0}"
gum style --border rounded --border-foreground "${__COLOR_ACCENT_8}" --padding "0 2" --bold "Installing Malbox CLI"
__status=$?
printf '%s\n' ""
gum log --level info --prefix.foreground "${__COLOR_ACCENT_8}" "Architecture: ${arch_28}"
__status=$?
gum log --level info --prefix.foreground "${__COLOR_ACCENT_8}" "Install to:   ${install_dir_52}/${__BINARY_NAME_4}"
__status=$?
printf '%s\n' ""
choose_channel__43_v0 
channel_59="${ret_choose_channel43_v0}"
printf '%s\n' ""
test -f "${install_dir_52}/${__BINARY_NAME_4}"
__status=$?
if [ "${__status}" = 0 ]; then
    command_21="$(printenv MALBOX_FORCE || true)"
    __status=$?
    force_60="${command_21}"
    is_interactive__42_v0 
    interactive_61="${ret_is_interactive42_v0}"
    # Non-interactive (or forced) installs overwrite silently so re-running
    # the bootstrap to upgrade works unattended.
    if [ "$(( $([ "_${force_60}" == "_1" ]; echo $?) && $([ "_${interactive_61}" != "_yes" ]; echo $?) ))" != 0 ]; then
        gum confirm --prompt.foreground "${__COLOR_ACCENT_8}" --selected.background "${__COLOR_ACCENT_8}" --unselected.foreground "${__COLOR_FAINT_12}" "Malbox CLI already exists at ${install_dir_52}/${__BINARY_NAME_4}. Overwrite?"
        __status=$?
        if [ "${__status}" != 0 ]; then
            gum log --level info --prefix.foreground "${__COLOR_ACCENT_8}" "Installation cancelled."
            __status=$?
            cleanup_gum__38_v0 
            exit 0
        fi
    fi
fi
fetch_release__44_v0 "${channel_59}"
tag_70="${ret_fetch_release44_v0}"
asset_name_71="malboxctl-${tag_70}-${arch_28}.tar.gz"
download_binary__45_v0 "${tag_70}" "${asset_name_71}"
tmp_dir_82="${ret_download_binary45_v0}"
verify_checksum__46_v0 "${tmp_dir_82}" "${asset_name_71}"
install_binary__47_v0 "${tmp_dir_82}" "${asset_name_71}" "${install_dir_52}"
printf '%s\n' ""
gum style --foreground "${__COLOR_SUCCESS_9}" --bold "Malbox CLI installed successfully!"
__status=$?
gum style --foreground "${__COLOR_FAINT_12}" "Run malboxctl install to set up Malbox."
__status=$?
cleanup_gum__38_v0 
