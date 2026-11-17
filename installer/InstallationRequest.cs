using System;
using System.IO;

namespace ColorfulControlCenter.Installer
{
    internal sealed class InstallationRequest
    {
        internal string Directory { get; private set; }
        internal static InstallationRequest Create(string value, InstallationRequest previous = null)
        {
            if (String.IsNullOrWhiteSpace(value) || value.Length > 220
                || value.Length < 3 || !Char.IsLetter(value[0]) || value[1] != ':'
                || (value[2] != '\\' && value[2] != '/'))
                throw new ArgumentException("请选择本地磁盘中的完整文件夹路径，例如 D:\\Apps\\ColorfulControlCenter。");
            foreach (char character in value)
                if (Char.IsControl(character) || character == '"')
                    throw new ArgumentException("安装路径不能包含引号或控制字符。");
            if (value.IndexOfAny(new[] { '*', '?', '<', '>', '|' }) >= 0 || value.Substring(2).Contains(":"))
                throw new ArgumentException("安装路径包含无效字符，请选择有效文件夹。");
            foreach (string segment in value.Replace('/', '\\').Substring(3).Split('\\'))
                if (segment.EndsWith(" ") || segment.EndsWith("."))
                    throw new ArgumentException("文件夹名称不能以空格或句点结尾。");
            string path = Path.GetFullPath(value).TrimEnd('\\', '/');
            if (path.Length <= 2)
                throw new ArgumentException("请选择独立的安装文件夹，不能直接安装到磁盘根目录。");
            bool replacing = File.Exists(Path.Combine(path, Product.ApplicationFileStem + ".exe"))
                && File.Exists(Path.Combine(path, "uninstall.exe"));
            if (File.Exists(path)) throw new ArgumentException("该路径是文件，请选择文件夹。");
            bool retry = previous != null && String.Equals(previous.Directory, path, StringComparison.OrdinalIgnoreCase);
            if (System.IO.Directory.Exists(path) && !replacing && !retry)
                using (var entries = System.IO.Directory.EnumerateFileSystemEntries(path).GetEnumerator())
                    if (entries.MoveNext())
                        throw new ArgumentException("该文件夹包含其他文件。请为控制台选择独立的空文件夹。");
            return new InstallationRequest { Directory = path };
        }

        internal string EngineArguments()
        {
            // NSIS /D consumes the rest of the command line, including spaces, without quotes.
            return "/S /D=" + Directory;
        }
    }
}
