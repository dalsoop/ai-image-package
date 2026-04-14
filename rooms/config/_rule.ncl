{
  validate = fun blob =>
    std.string.length blob.bin_name > 0
    && std.string.length blob.project_dir > 0
    && std.string.length blob.github_owner > 0
    && std.string.length blob.repo_prefix > 0
}
