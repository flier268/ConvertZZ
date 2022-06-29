using System;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Input;
using CommunityToolkit.Mvvm.Input;
using ConvertZZ.Core.Helpers;
using ConvertZZ.Core.Services.EncodingConverter;
using ConvertZZ.Core.Services.TextConverter;
using ConvertZZ.Moudle;
using PropertyChanged;

namespace ConvertZZ.ViewModels
{
    [AddINotifyPropertyChangedInterface]
    internal class MainWindowViewModel
    {
        public MainWindowViewModel()
        {
            #region SetupConverter

            TextConverter = new GoServiceTextConverter();
            CommonEncodingConverter = new();
            Big5ToGbkEncodingConverter = new(TextConverter);
            GbkToBig5EncodingConverter = new(TextConverter);
            UnicodeToHtmlHexCodeEncodingConverter = new();
            UnicodeToHtmlDexCodeEncodingConverter = new();
            HtmlCodeToUnicodeEncodingConverter = new();
            SymbolEncodingConverter = new();

            #endregion SetupConverter

            ShowHideWindowCommand = MessengerHelper.RegistInvokeCommandMessageAndReturnCommand(this, () =>
            {
                IsVisible = !IsVisible;
            }, EnumCommand.ShowHideWindowCommand);

            GbkToBig5Command = new AsyncRelayCommand(async () =>
            {
                long timeCost = await MessengerHelper.SendAsync<long>(EnumCommand.GbkToBig5Command);
                ShowTimeCost(timeCost);
            });
            Big5ToGbkCommand = new AsyncRelayCommand(async () =>
            {
                long timeCost = await MessengerHelper.SendAsync<long>(EnumCommand.Big5ToGbkCommand);
                ShowTimeCost(timeCost);
            });
            Unicode簡ToUnicode繁Command = new AsyncRelayCommand(async () =>
            {
                long timeCost = await MessengerHelper.SendAsync<long>(EnumCommand.Unicode簡ToUnicode繁Command);
                ShowTimeCost(timeCost);
            });
            Unicode繁ToUnicode簡Command = new AsyncRelayCommand(async () =>
            {
                long timeCost = await MessengerHelper.SendAsync<long>(EnumCommand.Unicode繁ToUnicode簡Command);
                ShowTimeCost(timeCost);
            });

            UnicodeToHtmlHexCodeCommand = new AsyncRelayCommand(async () =>
            {
                long timeCost = await MessengerHelper.SendAsync<long>(EnumCommand.UnicodeToHtmlHexCodeCommand);
                ShowTimeCost(timeCost);
            });
            UnicodeToHtmlDexCodeCommand = new AsyncRelayCommand(async () =>
            {
                long timeCost = await MessengerHelper.SendAsync<long>(EnumCommand.UnicodeToHtmlDexCodeCommand);
                ShowTimeCost(timeCost);
            });

            HtmlCodeToUnicodeCommand = new AsyncRelayCommand(async () =>
            {
                long timeCost = await MessengerHelper.SendAsync<long>(EnumCommand.HtmlCodeToUnicodeCommand);
                ShowTimeCost(timeCost);
            });
            CommonEncodingConvertCommand = new AsyncRelayCommand<string>(async (p) =>
            {
                long timeCost = await MessengerHelper.SendAsync<string, long>(EnumCommand.CommonEncodingConvertCommand, p);
                ShowTimeCost(timeCost);
            });
            FullSizeSymbolToHalfSymbolCommand = new AsyncRelayCommand(async () =>
            {
                long timeCost = await MessengerHelper.SendAsync<long>(EnumCommand.FullSizeSymbolToHalfSymbolCommand);
                ShowTimeCost(timeCost);
            });
            HalfSymbolToFullSizeSymbolCommand = new AsyncRelayCommand(async () =>
            {
                long timeCost = await MessengerHelper.SendAsync<long>(EnumCommand.HalfSymbolToFullSizeSymbolCommand);
                ShowTimeCost(timeCost);
            });

            ShowFileFolderConvertDialogCommand = new RelayCommand(() =>
            {
                MessengerHelper.Send(EnumCommand.ShowFileFolderConvertDialogCommand);
            });
            ShowClipboardConvertDialogCommand = new RelayCommand(() =>
            {
                MessengerHelper.Send(EnumCommand.ShowClipboardConvertDialogCommand);
            });
            ShowID3ConvertDialogCommand = new RelayCommand(() =>
            {
                MessengerHelper.Send(EnumCommand.ShowID3ConvertDialogCommand);
            });
            ShowAPEConvertDialogCommand = new RelayCommand(() =>
            {
                MessengerHelper.Send(EnumCommand.ShowAPEConvertDialogCommand);
            });
            ShowOGGConvertDialogCommand = new RelayCommand(() =>
            {
                MessengerHelper.Send(EnumCommand.ShowOGGConvertDialogCommand);
            });
            ShowAboutCommand = new RelayCommand(() =>
            {
                MessengerHelper.Send(EnumCommand.ShowAboutCommand);
            });

            ShowReportCommand = new RelayCommand(() =>
            {
                MessengerHelper.Send(EnumCommand.ShowReportCommand);
            });

            ShowSettingCommand = new RelayCommand(() =>
            {
                MessengerHelper.Send(EnumCommand.ShowSettingCommand);
            });
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

        private void ShowTimeCost(long timeCost)
        {
            if (timeCost > -1 && App.Settings.Prompt)
            {
                new Toast(string.Format("轉換完成\r\n耗時：{0} ms", timeCost)).Show();
            }
        }

        #region Converter

        private ITextConverter TextConverter { get; set; }
        private CommonEncodingConverter CommonEncodingConverter { get; set; }
        private GbkToBig5EncodingConverter GbkToBig5EncodingConverter { get; set; }
        private Big5ToGbkEncodingConverter Big5ToGbkEncodingConverter { get; set; }
        private UnicodeToHtmlDexCodeEncodingConverter UnicodeToHtmlDexCodeEncodingConverter { get; set; }
        private UnicodeToHtmlHexCodeEncodingConverter UnicodeToHtmlHexCodeEncodingConverter { get; set; }
        private HtmlCodeToUnicodeEncodingConverter HtmlCodeToUnicodeEncodingConverter { get; set; }
        private SymbolEncodingConverter SymbolEncodingConverter { get; set; }

        #endregion Converter

        public bool IsVisible { get; set; }
        public bool Topmost { get; set; } = true;
        public ICommand ShowHideWindowCommand { get; set; }
        public ICommand GbkToBig5Command { get; set; }
        public ICommand Big5ToGbkCommand { get; set; }
        public ICommand Unicode簡ToUnicode繁Command { get; set; }
        public ICommand Unicode繁ToUnicode簡Command { get; set; }
        public ICommand UnicodeToHtmlHexCodeCommand { get; set; }
        public ICommand UnicodeToHtmlDexCodeCommand { get; set; }
        public ICommand HtmlCodeToUnicodeCommand { get; set; }
        public ICommand CommonEncodingConvertCommand { get; set; }
        public ICommand HalfSymbolToFullSizeSymbolCommand { get; set; }
        public ICommand FullSizeSymbolToHalfSymbolCommand { get; set; }
        public ICommand ShowFileFolderConvertDialogCommand { get; set; }
        public ICommand ShowClipboardConvertDialogCommand { get; set; }
        public ICommand ShowID3ConvertDialogCommand { get; set; }
        public ICommand ShowAPEConvertDialogCommand { get; set; }
        public ICommand ShowOGGConvertDialogCommand { get; set; }
        public ICommand ShowAboutCommand { get; set; }
        public ICommand ShowReportCommand { get; set; }
        public ICommand ShowSettingCommand { get; set; }
    }
}