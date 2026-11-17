using System;
using System.Diagnostics;
using System.IO;
using System.Reflection;
using System.Security.Cryptography;

internal static class VerifyPackage
{
    private static int Main(string[] args)
    {
        try
        {
            var assembly = Assembly.ReflectionOnlyLoadFrom(Path.GetFullPath(args[0]));
            var product = assembly.GetType("ColorfulControlCenter.Installer.Product", true);
            string version = Constant(product, "Version");
            var native = FileVersionInfo.GetVersionInfo(args[0]);
            Require(native.ProductVersion == version && native.FileVersion == version + ".0", "Version resources");
            Require(native.ProductName == Constant(product, "ProductName"), "Product name");
            Require(assembly.GetName().ProcessorArchitecture == ProcessorArchitecture.Amd64, "AMD64 wrapper");
            CheckResource(assembly, "CCC.Engine", args[1]);
            CheckResource(assembly, "CCC.Palette", Path.Combine(args[3], "Palette.xaml"));
            CheckResource(assembly, "CCC.Theme", "installer/Theme.xaml");
            CheckResource(assembly, "CCC.Artwork", "installer/assets/installer-artwork.png");
            CheckResource(assembly, "CCC.Icon", "src-tauri/icons/128x128@2x.png");
            Require(Constant(product, "EngineSha256") == FileHash(args[1]), "Engine digest metadata");
            Require(Constant(product, "ApplicationSha256") == FileHash(args[2]), "Application digest metadata");
            Console.WriteLine("Verified AMD64 wrapper, version metadata, all 5 embedded resources, and engine/application digests.");
            return 0;
        }
        catch (Exception error) { Console.Error.WriteLine(error); return 1; }
    }

    private static string Constant(Type product, string name)
    {
        return (string)product.GetField(name, BindingFlags.Static | BindingFlags.NonPublic).GetRawConstantValue();
    }

    private static void CheckResource(Assembly assembly, string name, string path)
    {
        using (var resource = assembly.GetManifestResourceStream(name))
        {
            Require(resource != null, "Missing resource: " + name);
            Require(Hash(resource) == FileHash(path), "Embedded resource differs: " + name);
        }
    }

    private static string FileHash(string path)
    {
        using (var stream = File.OpenRead(path)) return Hash(stream);
    }

    private static string Hash(Stream stream)
    {
        using (var sha = SHA256.Create())
            return BitConverter.ToString(sha.ComputeHash(stream)).Replace("-", "").ToLowerInvariant();
    }

    private static void Require(bool condition, string detail)
    {
        if (!condition) throw new InvalidDataException(detail);
    }
}
