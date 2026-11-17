using System;
using System.Collections;
using System.Text;

namespace ColorfulControlCenter.Installer
{
    internal sealed class InstallFailure : Exception
    {
        internal string LogPath { get; private set; }
        internal InstallFailure(Exception cause, string logPath)
            : base(cause.Message, cause) { LogPath = logPath; }
    }

    internal static class InstallerDiagnostics
    {
        internal static string Describe(Exception error)
        {
            var text = new StringBuilder(error.ToString());
            for (Exception cause = error; cause != null; cause = cause.InnerException)
                foreach (DictionaryEntry item in cause.Data)
                    text.AppendLine().Append(item.Key).Append(": ").Append(item.Value);
            var install = error as InstallFailure;
            if (install != null) text.AppendLine().Append("诊断日志：").Append(install.LogPath);
            return text.ToString();
        }
    }
}
