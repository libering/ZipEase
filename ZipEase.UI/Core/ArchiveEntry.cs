namespace ZipEase.UI.Core
{
    public class ArchiveEntry
    {
        public string FileName { get; set; } = string.Empty;
        public long Size { get; set; }
        public string FileType { get; set; } = string.Empty;
        public string FormattedSize { get; set; } = string.Empty;
        public bool IsDirectory { get; set; }
    }
}
