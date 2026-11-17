using System;
using System.Diagnostics;
using System.IO;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Threading.Tasks;

namespace ColorfulControlCenter.Installer
{
    internal sealed class InstallProgress
    {
        internal string Phase { get; set; }
        internal double? Fraction { get; set; }
    }

    internal sealed class InstallerEngine
    {
        internal async Task Run(InstallationRequest request, IProgress<InstallProgress> progress)
        {
            string staging = Path.Combine(Path.GetTempPath(), "ccc-setup-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(staging);
            string logPath = Path.Combine(staging, "install.log");
            string engine = Path.Combine(staging, "engine.exe");
            using (var log = new StreamWriter(logPath, false, new UTF8Encoding(false)) { AutoFlush = true })
            {
                log.WriteLine(DateTimeOffset.Now + " | " + Product.Version + " | " + request.Directory);
                try
                {
                    await Extract(engine, progress);
                    if (!String.Equals(Hash(engine), Product.EngineSha256, StringComparison.OrdinalIgnoreCase))
                        throw new IOException("安装引擎校验失败，请重新下载安装包。");
                    log.WriteLine("Payload verified. Starting NSIS.");
                    progress.Report(new InstallProgress { Phase = "正在安装程序与运行组件…" });
                    int code = await RunProcess(engine, request.EngineArguments());
                    log.WriteLine("NSIS exit code: " + code);
                    if (code != 0) throw new IOException("安装引擎退出，错误代码 " + code
                        + "。请确认控制台已退出、目录可写，且网络可用于准备 WebView2，然后重试。");
                    progress.Report(new InstallProgress { Phase = "正在核对安装文件…" });
                    VerifyInstallation(request.Directory);
                    log.WriteLine("Application hash and uninstaller verified. Installation complete.");
                }
                catch (Exception error)
                {
                    log.WriteLine(InstallerDiagnostics.Describe(error));
                    throw new InstallFailure(error, logPath);
                }
                finally
                {
                    try { if (File.Exists(engine)) File.Delete(engine); }
                    catch (Exception error) { log.WriteLine("Payload cleanup: " + error.Message); }
                }
            }
        }

        private static async Task Extract(string outputPath, IProgress<InstallProgress> progress)
        {
            using (var input = Assembly.GetExecutingAssembly().GetManifestResourceStream("CCC.Engine"))
            {
                if (input == null) throw new InvalidDataException("安装包缺少引擎，请重新下载。");
                using (var output = File.Create(outputPath))
                {
                    var buffer = new byte[81920];
                    long copied = 0;
                    int count;
                    while ((count = await input.ReadAsync(buffer, 0, buffer.Length)) > 0)
                    {
                        await output.WriteAsync(buffer, 0, count);
                        copied += count;
                        progress.Report(new InstallProgress {
                            Phase = "正在准备安装文件…", Fraction = (double)copied / input.Length,
                        });
                    }
                }
            }
        }

        private static Task<int> RunProcess(string executable, string arguments)
        {
            return Task.Run(() => {
                using (var process = Process.Start(new ProcessStartInfo(executable, arguments) {
                    UseShellExecute = false, CreateNoWindow = true,
                    WorkingDirectory = Path.GetDirectoryName(executable),
                }))
                {
                    if (process == null) throw new IOException("安装引擎未能启动。");
                    process.WaitForExit();
                    return process.ExitCode;
                }
            });
        }

        internal static void VerifyInstallation(string directory)
        {
            string executable = Path.Combine(directory, Product.ApplicationFileStem + ".exe");
            if (!File.Exists(executable)) throw new IOException("安装后未找到主程序，请重试安装。");
            if (!File.Exists(Path.Combine(directory, "uninstall.exe")))
                throw new IOException("安装后未找到卸载程序，请重试安装。");
            string actual = Hash(executable);
            if (String.Equals(actual, Product.ApplicationSha256, StringComparison.OrdinalIgnoreCase)) return;
            var error = new IOException("安装后的程序与安装包内容不一致，请重试安装。");
            error.Data["ApplicationPath"] = executable;
            error.Data["ExpectedSHA256"] = Product.ApplicationSha256;
            error.Data["ActualSHA256"] = actual;
            throw error;
        }

        internal static string Hash(string path)
        {
            using (var sha = SHA256.Create())
            using (var stream = File.OpenRead(path))
                return BitConverter.ToString(sha.ComputeHash(stream)).Replace("-", "").ToLowerInvariant();
        }
    }
}
