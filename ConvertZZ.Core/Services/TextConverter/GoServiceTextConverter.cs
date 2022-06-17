using System.Diagnostics;
using System.Text.Json;
using ConvertZZ.Core.Helpers;

namespace ConvertZZ.Core.Services.TextConverter
{
    public class GoServiceTextConverter : ITextConverter
    {
        public GoServiceTextConverter()
        {
            JsonSerializerOptions = new(JsonSerializerDefaults.Web);
        }

        public GoServiceTextConverter(int port) : this()
        {
            Port = port;
        }

        public int Port { get; set; } = 8080;
        private HttpClient Client { get; } = new();
        private JsonSerializerOptions JsonSerializerOptions { get; }

        public async Task<bool> IsEnable()
        {
            string version = await GetVersion();
            return new Version(version) > Version.Parse("1.0.0.0");
        }

        public async Task<string> GetVersion()
        {
            var uriBuilder = new UriBuilder() { Host = "127.0.0.1", Scheme = Uri.UriSchemeHttp, Port = Port };
            uriBuilder.Path = $"/version";
            return await Client.GetStringAsync(uriBuilder.Uri);
        }

        public string Convert(string text, ETextConvertMode mode)
        {
            return ConvertAsync(text, mode).Result;
        }

        public async Task<string> ConvertAsync(string text, ETextConvertMode mode)
        {
            if (mode == ETextConvertMode.None)
                return text;
            try
            {
                var json = await Client.GetStringAsync(CreateUri(text, mode));
                var response = JsonSerializer.Deserialize<Response>(json, JsonSerializerOptions);
                if (response?.Status == "ok" && response?.Output is not null)
                    return response.Output;
            }
            catch (HttpRequestException)
            {
                StartService();
                try
                {
                    var json = await Client.GetStringAsync(CreateUri(text, mode));
                    var response = JsonSerializer.Deserialize<Response>(json, JsonSerializerOptions);
                    if (response?.Status == "ok" && response?.Output is not null)
                        return response.Output;
                }
                catch { }
            }
            return await Task.FromResult(text);
        }

        private Uri CreateUri(string text, ETextConvertMode mode)
        {
            var uriBuilder = new UriBuilder() { Host = "127.0.0.1", Scheme = Uri.UriSchemeHttp, Port = Port };
            uriBuilder.Path = $"/api/v1/textConverter/convert/{mode}/{text}";
            return uriBuilder.Uri;
        }

        private int? GetServicePort()
        {
            var ps = TcpIpHelper.GetAllProcesses();
            Process[] serviceProcesses = Process.GetProcessesByName("ConvertZZ.Service");
            foreach (var p in serviceProcesses)
            {
                var prc = ps.FirstOrDefault(x => x.PID == p.Id);
                if (prc is not null)
                    return prc.Port;
            }
            return null;
        }

        public GoServiceTextConverter StartService()
        {
            int? port = GetServicePort();
            if (port is null)
            {
                Port = TcpIpHelper.GetRandomUnusedPort();
                var pStartInfo = new ProcessStartInfo();
                pStartInfo.FileName = "./ConvertZZ.Service.exe";
                pStartInfo.Arguments = $"-p {Port}";
                pStartInfo.UseShellExecute = false;
                pStartInfo.CreateNoWindow = true;
                Process? p = Process.Start(pStartInfo);
                p?.WaitForExit(7000);
            }
            else
            {
                Port = (int)port;
            }
            return this;
        }

        public void StopService()
        {
            foreach (var p in Process.GetProcessesByName("ConvertZZ.Service"))
            {
                p.CloseMainWindow();
                //p.Kill();
            }
        }

        public class Response
        {
            public string? Output { get; set; }
            public string? Status { get; set; }
            public string? Message { get; set; }
        }
    }
}