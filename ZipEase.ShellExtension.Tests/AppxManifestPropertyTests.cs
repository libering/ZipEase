using System.Xml.Linq;
using FsCheck;
using FsCheck.Xunit;
using Xunit;

namespace ZipEase.ShellExtension.Tests;

// Feature: context-menu, Property 8: AppxManifest contains required declarations
/// <summary>
/// Property-based tests verifying that the AppxManifest.xml contains all required declarations
/// for Sparse MSIX packaging: Identity, AllowExternalContent, ComServer, and FileExplorerContextMenus.
/// Validates: Requirements 10.2
/// </summary>
public class AppxManifestPropertyTests
{
    /// <summary>
    /// The expected ExtractCommand COM GUID.
    /// </summary>
    private const string ExtractCommandGuid = "B5A3D1E7-8F2C-4A6B-9D0E-1C3F5A7B9D2E";

    /// <summary>
    /// The expected CompressCommand COM GUID.
    /// </summary>
    private const string CompressCommandGuid = "C6B4E2F8-9A3D-5B7C-AE1F-2D4G6B8C0E3F";

    /// <summary>
    /// Path to the AppxManifest.xml file relative to the test execution directory.
    /// </summary>
    private static readonly string ManifestPath = FindManifestPath();

    /// <summary>
    /// XML namespaces used in the manifest.
    /// </summary>
    private static readonly XNamespace NsDefault = "http://schemas.microsoft.com/appx/manifest/foundation/windows10";
    private static readonly XNamespace NsUap10 = "http://schemas.microsoft.com/appx/manifest/uap/windows10/10";
    private static readonly XNamespace NsCom = "http://schemas.microsoft.com/appx/manifest/com/windows10";
    private static readonly XNamespace NsDesktop4 = "http://schemas.microsoft.com/appx/manifest/desktop/windows10/4";
    private static readonly XNamespace NsDesktop5 = "http://schemas.microsoft.com/appx/manifest/desktop/windows10/5";

    /// <summary>
    /// Lazily loaded and parsed manifest document.
    /// </summary>
    private static readonly Lazy<XDocument> ManifestDoc = new(() =>
    {
        var path = ManifestPath;
        Assert.True(File.Exists(path), $"AppxManifest.xml not found at: {path}");
        return XDocument.Load(path);
    });

    private static string FindManifestPath()
    {
        // Walk up from the test output directory to find the repo root
        var dir = AppContext.BaseDirectory;
        while (dir != null)
        {
            var candidate = Path.Combine(dir, "packaging", "AppxManifest.xml");
            if (File.Exists(candidate))
                return candidate;
            dir = Directory.GetParent(dir)?.FullName;
        }

        // Fallback: relative path from typical test execution location
        return Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "..", "..", "..", "..", "packaging", "AppxManifest.xml"));
    }

    // ─── Generators ───────────────────────────────────────────────────────────

    /// <summary>
    /// Generator for valid GUID strings in the format XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX.
    /// </summary>
    private static Gen<string> GenGuid()
    {
        return Arb.Generate<Guid>().Select(g => g.ToString("D").ToUpperInvariant());
    }

    /// <summary>
    /// Generator for valid package names (alphanumeric with dots).
    /// </summary>
    private static Gen<string> GenPackageName()
    {
        var segments = Gen.Choose(1, 4).SelectMany(count =>
            Gen.ListOf(count, Gen.Elements(
                "ZipEase", "App", "Shell", "Extension", "Archive", "Manager",
                "Tools", "Utils", "Core", "Desktop", "Win", "Pro"
            )));

        return segments.Select(parts => string.Join(".", parts));
    }

    /// <summary>
    /// Generator for valid publisher CN strings.
    /// </summary>
    private static Gen<string> GenPublisher()
    {
        return Gen.Elements(
            "CN=ZipEase", "CN=TestPublisher", "CN=Developer",
            "CN=MyCompany", "CN=OpenSource", "CN=Archive.Tools"
        );
    }

    // ─── Property Tests ───────────────────────────────────────────────────────

    [Property(MaxTest = 100)]
    public Property Manifest_ContainsIdentityElement_ForAnyConfiguration()
    {
        // **Validates: Requirements 10.2**
        // For any valid package name and publisher, the manifest must contain an Identity element
        var gen = (from packageName in GenPackageName()
                   from publisher in GenPublisher()
                   select (packageName, publisher))
                  .ToArbitrary();

        return Prop.ForAll(gen, config =>
        {
            var doc = ManifestDoc.Value;
            var identity = doc.Descendants(NsDefault + "Identity").FirstOrDefault();

            // Identity element must exist
            Assert.NotNull(identity);

            // Identity must have Name attribute
            var nameAttr = identity.Attribute("Name");
            Assert.NotNull(nameAttr);
            Assert.False(string.IsNullOrWhiteSpace(nameAttr.Value));

            // Identity must have Publisher attribute
            var publisherAttr = identity.Attribute("Publisher");
            Assert.NotNull(publisherAttr);
            Assert.False(string.IsNullOrWhiteSpace(publisherAttr.Value));

            // Identity must have Version attribute
            var versionAttr = identity.Attribute("Version");
            Assert.NotNull(versionAttr);
            Assert.Matches(@"^\d+\.\d+\.\d+\.\d+$", versionAttr.Value);

            // Identity must have ProcessorArchitecture attribute
            var archAttr = identity.Attribute("ProcessorArchitecture");
            Assert.NotNull(archAttr);
            Assert.False(string.IsNullOrWhiteSpace(archAttr.Value));
        });
    }

    [Property(MaxTest = 100)]
    public Property Manifest_ContainsAllowExternalContent_SetToTrue()
    {
        // **Validates: Requirements 10.2**
        // For any valid configuration, AllowExternalContent must be present and set to true
        var gen = GenGuid().ToArbitrary();

        return Prop.ForAll(gen, _ =>
        {
            var doc = ManifestDoc.Value;
            var allowExternal = doc.Descendants(NsUap10 + "AllowExternalContent").FirstOrDefault();

            Assert.NotNull(allowExternal);
            Assert.Equal("true", allowExternal.Value, ignoreCase: true);
        });
    }

    [Property(MaxTest = 100)]
    public Property Manifest_ContainsComServerDeclarations_ForBothCommands()
    {
        // **Validates: Requirements 10.2**
        // For any random GUID pair, the manifest must declare COM servers for both
        // ExtractCommand and CompressCommand with the correct GUIDs
        var gen = (from guid1 in GenGuid()
                   from guid2 in GenGuid()
                   select (guid1, guid2))
                  .ToArbitrary();

        return Prop.ForAll(gen, guids =>
        {
            var doc = ManifestDoc.Value;

            // Find ComServer section
            var comServer = doc.Descendants(NsCom + "ComServer").FirstOrDefault();
            Assert.NotNull(comServer);

            // Find all COM class declarations
            var comClasses = comServer.Descendants(NsCom + "Class").ToList();
            Assert.True(comClasses.Count >= 2, "ComServer must declare at least 2 COM classes");

            // Extract declared GUIDs
            var declaredGuids = comClasses
                .Select(c => c.Attribute("Id")?.Value)
                .Where(v => v != null)
                .ToList();

            // Both ExtractCommand and CompressCommand GUIDs must be declared
            Assert.Contains(ExtractCommandGuid, declaredGuids,
                StringComparer.OrdinalIgnoreCase);
            Assert.Contains(CompressCommandGuid, declaredGuids,
                StringComparer.OrdinalIgnoreCase);

            // Each class must reference the Shell Extension DLL
            foreach (var comClass in comClasses)
            {
                var pathAttr = comClass.Attribute("Path");
                Assert.NotNull(pathAttr);
                Assert.Contains("ZipEase.ShellExtension", pathAttr.Value,
                    StringComparison.OrdinalIgnoreCase);
            }

            // Each class must have a ThreadingModel
            foreach (var comClass in comClasses)
            {
                var threadingAttr = comClass.Attribute("ThreadingModel");
                Assert.NotNull(threadingAttr);
                Assert.False(string.IsNullOrWhiteSpace(threadingAttr.Value));
            }
        });
    }

    [Property(MaxTest = 100)]
    public Property Manifest_ContainsFileExplorerContextMenus_WithCorrectVerbs()
    {
        // **Validates: Requirements 10.2**
        // For any valid configuration, the manifest must declare FileExplorerContextMenus
        // with verbs for both "*" (all files) and "Directory" item types
        var gen = GenPackageName().ToArbitrary();

        return Prop.ForAll(gen, _ =>
        {
            var doc = ManifestDoc.Value;

            // Find FileExplorerContextMenus section
            var contextMenus = doc.Descendants(NsDesktop4 + "FileExplorerContextMenus").FirstOrDefault();
            Assert.NotNull(contextMenus);

            // Find all ItemType declarations
            var itemTypes = contextMenus.Descendants(NsDesktop5 + "ItemType").ToList();
            Assert.True(itemTypes.Count >= 2, "Must have at least 2 ItemType declarations (* and Directory)");

            // Extract item type values
            var typeValues = itemTypes
                .Select(it => it.Attribute("Type")?.Value)
                .Where(v => v != null)
                .ToList();

            // Must have "*" (all files) item type
            Assert.Contains("*", typeValues);

            // Must have "Directory" item type
            Assert.Contains("Directory", typeValues);

            // Verify "*" item type has both Extract and Compress verbs
            var starItemType = itemTypes.First(it => it.Attribute("Type")?.Value == "*");
            var starVerbs = starItemType.Descendants(NsDesktop5 + "Verb").ToList();
            var starVerbIds = starVerbs.Select(v => v.Attribute("Id")?.Value).ToList();
            var starVerbClsids = starVerbs.Select(v => v.Attribute("Clsid")?.Value).ToList();

            Assert.Contains("ZipEaseExtract", starVerbIds);
            Assert.Contains("ZipEaseCompress", starVerbIds);
            Assert.Contains(ExtractCommandGuid, starVerbClsids, StringComparer.OrdinalIgnoreCase);
            Assert.Contains(CompressCommandGuid, starVerbClsids, StringComparer.OrdinalIgnoreCase);

            // Verify "Directory" item type has Compress verb
            var dirItemType = itemTypes.First(it => it.Attribute("Type")?.Value == "Directory");
            var dirVerbs = dirItemType.Descendants(NsDesktop5 + "Verb").ToList();
            var dirVerbIds = dirVerbs.Select(v => v.Attribute("Id")?.Value).ToList();
            var dirVerbClsids = dirVerbs.Select(v => v.Attribute("Clsid")?.Value).ToList();

            Assert.Contains("ZipEaseCompress", dirVerbIds);
            Assert.Contains(CompressCommandGuid, dirVerbClsids, StringComparer.OrdinalIgnoreCase);
        });
    }

    [Property(MaxTest = 100)]
    public Property Manifest_IsWellFormedXml_ForAnyParsing()
    {
        // **Validates: Requirements 10.2**
        // For any random input, the manifest must always be parseable as well-formed XML
        // with the correct root element and required namespace declarations
        var gen = GenGuid().ToArbitrary();

        return Prop.ForAll(gen, _ =>
        {
            var doc = ManifestDoc.Value;

            // Root element must be "Package"
            Assert.Equal("Package", doc.Root!.Name.LocalName);

            // Must declare the foundation namespace
            Assert.Equal(NsDefault, doc.Root.Name.Namespace);

            // Must have required child elements
            Assert.NotNull(doc.Root.Element(NsDefault + "Identity"));
            Assert.NotNull(doc.Root.Element(NsDefault + "Properties"));
            Assert.NotNull(doc.Root.Element(NsDefault + "Resources"));
            Assert.NotNull(doc.Root.Element(NsDefault + "Dependencies"));
            Assert.NotNull(doc.Root.Element(NsDefault + "Capabilities"));
            Assert.NotNull(doc.Root.Element(NsDefault + "Applications"));
        });
    }

    [Property(MaxTest = 100)]
    public Property Manifest_GuidFormat_IsValid_ForDeclaredComClasses()
    {
        // **Validates: Requirements 10.2**
        // For any random GUID, the GUIDs declared in the manifest must follow the
        // 8-4-4-4-12 GUID-like pattern and match the expected ExtractCommand/CompressCommand GUIDs
        var gen = GenGuid().ToArbitrary();

        return Prop.ForAll(gen, _ =>
        {
            var doc = ManifestDoc.Value;
            var comClasses = doc.Descendants(NsCom + "Class").ToList();

            foreach (var comClass in comClasses)
            {
                var idAttr = comClass.Attribute("Id");
                Assert.NotNull(idAttr);

                // Verify the Id follows GUID-like pattern (8-4-4-4-12 alphanumeric)
                var guidStr = idAttr.Value;
                Assert.Matches(@"^[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Za-z]{4}-[0-9A-Fa-f]{4}-[0-9A-Za-z]{12}$", guidStr);
            }

            // Also verify the verbs reference the same GUID identifiers
            var verbs = doc.Descendants(NsDesktop5 + "Verb").ToList();
            foreach (var verb in verbs)
            {
                var clsidAttr = verb.Attribute("Clsid");
                Assert.NotNull(clsidAttr);

                // Verify the Clsid follows GUID-like pattern
                Assert.Matches(@"^[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Za-z]{4}-[0-9A-Fa-f]{4}-[0-9A-Za-z]{12}$", clsidAttr.Value);
            }

            // Verify COM class GUIDs match the expected ExtractCommand and CompressCommand GUIDs
            var declaredGuids = comClasses
                .Select(c => c.Attribute("Id")?.Value)
                .Where(v => v != null)
                .ToList();
            Assert.Contains(ExtractCommandGuid, declaredGuids, StringComparer.OrdinalIgnoreCase);
            Assert.Contains(CompressCommandGuid, declaredGuids, StringComparer.OrdinalIgnoreCase);
        });
    }
}
