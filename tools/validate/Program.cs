// Validates .docx/.docm/.dotx/.dotm files against the Office schemas with
// Microsoft's Open XML SDK (the same validation Word's file-format code follows).
//   dotnet run -c Release -- [--format Office2007|Office2010|...|Microsoft365] file.docx...
// Exit code 1 when any file has errors.
using DocumentFormat.OpenXml;
using DocumentFormat.OpenXml.Packaging;
using DocumentFormat.OpenXml.Validation;

var format = FileFormatVersions.Microsoft365;
var files = new List<string>();
for (int i = 0; i < args.Length; i++)
{
    if (args[i] == "--format" && i + 1 < args.Length) format = Enum.Parse<FileFormatVersions>(args[++i]);
    else files.Add(args[i]);
}
int bad = 0;
foreach (var f in files)
{
    try
    {
        using var doc = WordprocessingDocument.Open(f, false);
        var errors = new OpenXmlValidator(format).Validate(doc).ToList();
        Console.WriteLine($"{(errors.Count == 0 ? "OK  " : "FAIL")} {Path.GetFileName(f)}: {errors.Count} error(s)");
        foreach (var e in errors.Take(40))
            Console.WriteLine($"     {e.Part?.Uri} {e.Path?.XPath}\n       {e.ErrorType}: {e.Description}");
        if (errors.Count > 0) bad++;
    }
    catch (Exception ex)
    {
        Console.WriteLine($"OPEN-FAIL {Path.GetFileName(f)}: {ex.GetType().Name}: {ex.Message}");
        bad++;
    }
}
return bad > 0 ? 1 : 0;
