using System;
using System.Collections;
using System.IO;
using System.Text;

namespace ColorfulControlCenter.Installer
{
    internal sealed class InstallFailure : Exception
    {
        internal string LogPath { get; private set; }
        internal int? EngineExitCode
        {
            get
            {
                var engine = InnerException as EngineExitFailure;
                return engine == null ? (int?)null : engine.Code;
            }
        }
        internal InstallFailure(Exception cause, string logPath)
            : base(cause.Message, cause) { LogPath = logPath; }
    }

    internal sealed class EngineExitFailure : IOException
    {
        internal const string ControllerExitInstructions = "请从系统托盘退出控制台后重试安装。";
        internal int Code { get; private set; }

        internal EngineExitFailure(int code) : base(MessageFor(code))
        {
            Code = code;
            Data["EngineExitCode"] = code;
        }

        private static string MessageFor(int code)
        {
            if (code == Product.ControllerRunningExitCode)
                return "控制台仍在使用安装文件。" + ControllerExitInstructions;
            if (code == Product.ProcessCheckFailedExitCode)
                return "无法确认控制台是否已退出。可从系统托盘确认后重试。";
            return "安装程序已中止。请重试，或查看诊断详情。";
        }
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
