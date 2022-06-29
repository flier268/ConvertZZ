using System;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using CommunityToolkit.Mvvm.Messaging;
using ConvertZZ.Core;
using ConvertZZ.Core.Helpers;
using ConvertZZ.Core.Messages;
using ConvertZZ.Core.Services.EncodingConverter;
using ConvertZZ.Moudle;
using ConvertZZ.Views;

namespace ConvertZZ.Services
{
    public class MainWindowService
    {
        public MainWindowService()
        {
        }

        public void RegistMessenger()
        {
            MessengerHelper.RegistInvokeCommandMessage(this, async () =>
            {
                var result = await ConvertClipboardTextAsync(async (clip, token) =>
                {
                    if (App.Settings.RecognitionEncoding)
                    {
                        EncodingAnalyzer.EncodingType encodingtype = EncodingAnalyzer.Analyze(clip);
                        if (encodingtype == EncodingAnalyzer.EncodingType.BIG5 || encodingtype == EncodingAnalyzer.EncodingType.BIG5AsGBK)
                            if (MessageBox.Show("編碼似乎已是Big5，繼續轉換?", "警告", MessageBoxButton.YesNo) == MessageBoxResult.No)
                            {
                                token.Cancel();
                                return await Task.FromResult((default(string), -1));
                            }
                    }
                    return await StopWatchTask.RunAsync(async () =>
                    {
                        return await App.Instance.GbkToBig5EncodingConverter.ConvertAsync(clip);
                    });
                });
                return await Task.FromResult(result.timeCost);
            }, EnumCommand.GbkToBig5Command);

            MessengerHelper.RegistInvokeCommandMessage(this, async () =>
            {
                var result = await ConvertClipboardTextAsync(async (clip, token) =>
                {
                    if (App.Settings.RecognitionEncoding)
                    {
                        EncodingAnalyzer.EncodingType encodingtype = EncodingAnalyzer.Analyze(clip);
                        if (encodingtype == EncodingAnalyzer.EncodingType.GBK || encodingtype == EncodingAnalyzer.EncodingType.GBKAsBIG5)
                            if (MessageBox.Show("編碼似乎已是GBK，繼續轉換?", "警告", MessageBoxButton.YesNo) == MessageBoxResult.No)
                            {
                                token.Cancel();
                                return await Task.FromResult((default(string), -1));
                            }
                    }
                    return await StopWatchTask.RunAsync(async () =>
                    {
                        return await App.Instance.Big5ToGbkEncodingConverter.ConvertAsync(clip);
                    });
                });
                return await Task.FromResult(result.timeCost);
            }, EnumCommand.Big5ToGbkCommand);

            MessengerHelper.RegistInvokeCommandMessage(this, async () =>
            {
                var result = await ConvertClipboardTextAsync(async (clip, token) =>
                {
                    return await StopWatchTask.RunAsync(async () =>
                    {
                        return await App.Instance.TextConverter.ConvertAsync(clip, Core.Helpers.ETextConvertMode.S2T);
                    });
                });
                return await Task.FromResult(result.timeCost);
            }, EnumCommand.Unicode簡ToUnicode繁Command);

            MessengerHelper.RegistInvokeCommandMessage(this, async () =>
            {
                var result = await ConvertClipboardTextAsync(async (clip, token) =>
                {
                    return await StopWatchTask.RunAsync(async () =>
                    {
                        return await App.Instance.TextConverter.ConvertAsync(clip, Core.Helpers.ETextConvertMode.T2S);
                    });
                });
                return await Task.FromResult(result.timeCost);
            }, EnumCommand.Unicode繁ToUnicode簡Command);
            MessengerHelper.RegistInvokeCommandMessage(this, () =>
            {
                var result = ConvertClipboardText((clip, token) =>
                {
                    return StopWatchTask.Run(() =>
                    {
                        return App.Instance.UnicodeToHtmlHexCodeEncodingConverter.Convert(clip);
                    });
                });
                return result.timeCost;
            }, EnumCommand.UnicodeToHtmlHexCodeCommand);
            MessengerHelper.RegistInvokeCommandMessage(this, () =>
            {
                var result = ConvertClipboardText((clip, token) =>
                {
                    return StopWatchTask.Run(() =>
                    {
                        return App.Instance.UnicodeToHtmlDexCodeEncodingConverter.Convert(clip);
                    });
                });
                return result.timeCost;
            }, EnumCommand.UnicodeToHtmlDexCodeCommand);
            MessengerHelper.RegistInvokeCommandMessage(this, () =>
            {
                var result = ConvertClipboardText((clip, token) =>
                {
                    return StopWatchTask.Run(() =>
                    {
                        return App.Instance.HtmlCodeToUnicodeEncodingConverter.Convert(clip);
                    });
                });
                return result.timeCost;
            }, EnumCommand.HtmlCodeToUnicodeCommand);
            MessengerHelper.RegistInvokeCommandMessage(this, (string? p) =>
            {
                var result = ConvertClipboardText((clip, token) =>
                {
                    return StopWatchTask.Run(() =>
                    {
                        string[] encodings = p.Split(">");
                        Encoding encoding1 = Encoding.GetEncoding(encodings[0]);
                        Encoding encoding2 = Encoding.GetEncoding(encodings[1]);
                        return App.Instance.CommonEncodingConverter.Convert(clip, encoding1, encoding2);
                    });
                });
                return result.timeCost;
            }, EnumCommand.CommonEncodingConvertCommand);

            MessengerHelper.RegistInvokeCommandMessage(this, () =>
            {
                var result = ConvertClipboardText((clip, token) =>
                {
                    return StopWatchTask.Run(() =>
                    {
                        return App.Instance.SymbolEncodingConverter.Convert(clip, SymbolEncodingConverter.SymbolConvertMode.FullSizeSymbolToHalfSymbol);
                    });
                });
                return result.timeCost;
            }, EnumCommand.FullSizeSymbolToHalfSymbolCommand);

            MessengerHelper.RegistInvokeCommandMessage(this, () =>
            {
                var result = ConvertClipboardText((clip, token) =>
                {
                    return StopWatchTask.Run(() =>
                    {
                        return App.Instance.SymbolEncodingConverter.Convert(clip, SymbolEncodingConverter.SymbolConvertMode.HalfSymbolToFullSizeSymbol);
                    });
                });
                return result.timeCost;
            }, EnumCommand.HalfSymbolToFullSizeSymbolCommand);

            MessengerHelper.RegistInvokeCommandMessageAndReturnCommand(this, () =>
            {
                WeakReferenceMessenger.Default.Send(new DialogHostMessage(EMode.File_FileName));
            }, EnumCommand.ShowFileFolderConvertDialogCommand);

            MessengerHelper.RegistInvokeCommandMessageAndReturnCommand(this, () =>
            {
                WeakReferenceMessenger.Default.Send(new DialogHostMessage(EMode.ClipBoard));
            }, EnumCommand.ShowClipboardConvertDialogCommand);
            MessengerHelper.RegistInvokeCommandMessageAndReturnCommand(this, () =>
           {
               WeakReferenceMessenger.Default.Send(new DialogHostMessage(EMode.AutioTag, EAudioFormat.ID3));
           }, EnumCommand.ShowID3ConvertDialogCommand);
            MessengerHelper.RegistInvokeCommandMessageAndReturnCommand(this, () =>
            {
                WeakReferenceMessenger.Default.Send(new DialogHostMessage(EMode.AutioTag, EAudioFormat.APE));
            }, EnumCommand.ShowAPEConvertDialogCommand);
            MessengerHelper.RegistInvokeCommandMessageAndReturnCommand(this, () =>
             {
                 WeakReferenceMessenger.Default.Send(new DialogHostMessage(EMode.AutioTag, EAudioFormat.OGG));
             }, EnumCommand.ShowOGGConvertDialogCommand);
            MessengerHelper.RegistInvokeCommandMessageAndReturnCommand(this, () =>
           {
               WeakReferenceMessenger.Default.Send(new WindowShowMessage(new Window_About()));
           }, EnumCommand.ShowAboutCommand);

            MessengerHelper.RegistInvokeCommandMessageAndReturnCommand(this, () =>
           {
               WeakReferenceMessenger.Default.Send(new WindowShowMessage(new Window_Report()));
           }, EnumCommand.ShowReportCommand);

            MessengerHelper.RegistInvokeCommandMessageAndReturnCommand(this, () =>
            {
                //Todo: hotkey
                //UnRegAllHotkey();
                //Topmost = false;
                WeakReferenceMessenger.Default.Send(new WindowShowMessage(new Window_Setting()));
                //Topmost = true;
                // RegAllHotkey();
            }, EnumCommand.ShowSettingCommand);
        }

        private (string str, long timeCost) ConvertClipboardText(Func<string, CancellationTokenSource, (string str, long timeCost)> func)
        {
            string clip = ClipBoardHelper.GetClipBoard_UnicodeText();
            CancellationTokenSource cancellationTokenSource = new();

            var result = func(clip, cancellationTokenSource);
            if (cancellationTokenSource.IsCancellationRequested)
                return result;

            ClipBoardHelper.SetClipBoard_UnicodeText(result.str);
            return result;
        }

        private async Task<(string str, long timeCost)> ConvertClipboardTextAsync(Func<string, CancellationTokenSource, Task<(string str, long timeCost)>> func)
        {
            string clip = ClipBoardHelper.GetClipBoard_UnicodeText();
            CancellationTokenSource cancellationTokenSource = new();

            var result = await func(clip, cancellationTokenSource);
            if (cancellationTokenSource.IsCancellationRequested)
                return result;

            ClipBoardHelper.SetClipBoard_UnicodeText(result.str);
            return result;
        }
    }
}