using System;
using System.Net.Http;
using System.Reflection;
using System.Text.RegularExpressions;

namespace ConvertZZ.Moudle
{
    public class UpdateChecker
    {
        public static VersionReport ChecktVersion()
        {
            try
            {
                using (var client = new HttpClient())
                {
                    var response = client.GetAsync("https://raw.githubusercontent.com/flier268/ConvertZZ/master/ConvertZZ/Properties/AssemblyInfo.cs").Result;

                    if (response.IsSuccessStatusCode)
                    {
                        var responseContent = response.Content;
                        string responseString = responseContent.ReadAsStringAsync().Result;
                        string pattern = @"^\[assembly: AssemblyVersion\(""(.*?)""\)\]";
                        var m = Regex.Match(responseString, pattern, RegexOptions.Multiline);
                        if (m.Success)
                        {
                            Version ver = new Version(m.Groups[1].ToString().ToString());
                            Version version = Assembly.GetEntryAssembly().GetName().Version;
                            int tm = version.CompareTo(ver);
                            VersionReport versionReport = new VersionReport();
                            return new VersionReport() { Current = version, Newst = ver, HaveNew = tm < 0 };
                        }
                    }
                }
            }
            catch { }
            return null;
        }
        public class VersionReport
        {
            public Version Current { get; set; }
            public Version Newst { get; set; }
            public bool HaveNew { get; set; }
        }
    }
}
